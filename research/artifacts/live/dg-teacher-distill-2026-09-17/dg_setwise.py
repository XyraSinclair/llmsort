"""DiffusionGemma as a setwise judge: template-in-canvas slot reads (after diffgemma PR #21/#23).

Mirrors experiments/examples/multi_criteria_setwise.rs: same prompts, ring windows (n, k=8,
overlap 2, 1 round, 2 presentations), same Huber fit, flip, halo and agreement numbers.
The judge does not generate. The answer line(s) go into the 256-token canvas with every
letter position replaced by a uniform-random token; one denoiser forward gives a PMF over
the k slot letters at every rank position. R reads with different hole noise are averaged.
"""
import argparse, itertools, json, math, os, random, sys, time
import numpy as np
import torch

SLOTS = "ABCDEFGHIJKL"
SYSTEM_SINGLE = "You are an expert subjective evaluator. You read a small set of entities in lettered slots, then an attribute. You answer with every slot letter exactly once, separated by spaces, ordered from the MOST of the attribute to the LEAST. Nothing else — no words, no punctuation, no explanation.\nExample: C A D B"
SYSTEM_JOINT = "You are an expert subjective evaluator. You read a small set of entities in lettered slots, then a numbered list of attributes. For EACH attribute, on its own line, you answer with the attribute number, a colon, then every slot letter exactly once, separated by spaces, ordered from the MOST of that attribute to the LEAST. One line per attribute, in the numbered order. Nothing else — no words, no explanation.\nExample:\n1: C A D B\n2: A C B D"
MODEL = "google/diffusiongemma-26B-A4B-it"
CACHE = "/data/models/hf"
QUANT = "fp8_e4m3 per-row"
LN_RATIO = math.log(2.78)


def escape(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace('"', "&quot;").replace("'", "&apos;")


def entity_block(texts, order):
    block = "<entities>\n"
    for slot, idx in enumerate(order):
        l = SLOTS[slot]
        block += f"<entity_{l}>\n{texts[idx]}\n</entity_{l}>\n"
    block += "</entities>"
    return block, ", ".join(SLOTS[s] for s in range(len(order)))


def prompt_single(texts, order, criterion):
    block, letters = entity_block(texts, order)
    return f"{block}\n\nCompare the entities by <attribute_name>: {escape(criterion)} </attribute_name>.\n\nOrder every slot from {{{letters}}} from MOST of the attribute to LEAST, every letter exactly once.\nanswer:"


def prompt_joint(texts, order, criteria):
    block, letters = entity_block(texts, order)
    attrs = "<attributes>\n" + "".join(f"<attribute_{i+1}>{escape(c)}</attribute_{i+1}>\n" for i, c in enumerate(criteria)) + "</attributes>"
    return f"{block}\n\nCompare the entities by each attribute in turn:\n{attrs}\n\nFor each attribute, on its own line `<number>: <letters>`, order every slot from {{{letters}}} from MOST of that attribute to LEAST, every letter exactly once. {len(criteria)} lines.\nanswer:"


def design(n, k, overlap, rounds, repeats, rng):
    stride = max(k - min(overlap, k - 1), 1)
    plans = []
    for _ in range(rounds):
        pool = list(range(n)); rng.shuffle(pool)
        windows = (n + stride - 1) // stride if n > k else 1
        for g in range(windows):
            order = [pool[(g * stride + j) % n] for j in range(k)] if n > k else list(pool)
            pres = []
            for p in range(max(repeats, 1)):
                o = list(order)
                if p > 0: rng.shuffle(o)
                pres.append(o)
            plans.append({"subset": sorted(order), "presentations": pres})
    return plans


def fit(n, obs):
    ridge, delta = 1e-3, 1.0
    w = np.ones(len(obs)); s = np.zeros(n); A = None
    hi = np.array([o[0] for o in obs], dtype=int); lo = np.array([o[1] for o in obs], dtype=int); r = np.array([o[2] for o in obs])
    for _ in range(6):
        A = np.eye(n) * ridge; b = np.zeros(n)
        np.add.at(A, (hi, hi), w); np.add.at(A, (lo, lo), w); np.add.at(A, (hi, lo), -w); np.add.at(A, (lo, hi), -w)
        np.add.at(b, hi, w * r); np.add.at(b, lo, -w * r)
        s = np.linalg.solve(A, b)
        res = np.abs(s[hi] - s[lo] - r)
        w = np.where(res <= delta, 1.0, delta / np.maximum(res, 1e-300))
    std = np.sqrt(np.maximum(np.diag(np.linalg.inv(A)) - 1.0 / (n * ridge), 0.0))
    parent = list(range(n))
    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]; x = parent[x]
        return x
    for a, b_ in zip(hi, lo): parent[find(a)] = find(b_)
    return {"scores": s.tolist(), "std": std.tolist(), "components": len({find(i) for i in range(n)})}


def ranks(v):
    idx = sorted(range(len(v)), key=lambda i: v[i]); r = [0.0] * len(v); i = 0
    while i < len(idx):
        j = i
        while j + 1 < len(idx) and v[idx[j + 1]] == v[idx[i]]: j += 1
        for t in idx[i:j + 1]: r[t] = (i + j) / 2.0
        i = j + 1
    return r


def spearman(a, b):
    ra, rb = np.array(ranks(a)), np.array(ranks(b))
    ra -= ra.mean(); rb -= rb.mean()
    return float((ra * rb).sum() / math.sqrt((ra ** 2).sum() * (rb ** 2).sum()))


def topk_overlap(a, b, k):
    top = lambda v: set(sorted(range(len(v)), key=lambda i: -v[i])[:k])
    return len(top(a) & top(b)) / k


def summarize(arm, names, n, traces, plans):
    per, fits = {}, []
    for name in names:
        obs, subset_ranks, parsed = [], {}, 0
        for t in traces:
            if name not in t["criteria"]: continue
            slots = t["parsed"][t["criteria"].index(name)]
            if slots is None: continue
            parsed += 1
            ranked = [t["order"][s] for s in slots]
            for a in range(len(ranked)):
                for b in range(a + 1, len(ranked)): obs.append((ranked[a], ranked[b], LN_RATIO))
            subset_ranks.setdefault(tuple(plans[t["plan"]]["subset"]), []).append({it: pos for pos, it in enumerate(ranked)})
        compared = flips = 0
        for pres in subset_ranks.values():
            for a in range(len(pres)):
                for b in range(a + 1, len(pres)):
                    for e in pres[a]:
                        for f in pres[a]:
                            if e >= f or e not in pres[b] or f not in pres[b]: continue
                            compared += 1
                            flips += (pres[a][e] < pres[a][f]) != (pres[b][e] < pres[b][f])
        f = fit(n, obs); fits.append(f["scores"])
        slot_stats = [h for t in traces if name in t["criteria"] for h in t["holes"][t["criteria"].index(name)]]
        per[name] = {"parsed": parsed, "flip": flips / compared if compared else None, "components": f["components"], "scores": f["scores"], "std": f["std"],
                     "label_mass_mean": float(np.mean([h["label_mass"] for h in slot_stats])), "label_mass_min": float(np.min([h["label_mass"] for h in slot_stats])),
                     "entropy_mean": float(np.mean([h["entropy"] for h in slot_stats])), "agreement_mean": float(np.mean([h["agreement"] for h in slot_stats])),
                     "stderr_mean": float(np.mean([h["stderr"] for h in slot_stats])), "argmax_is_perm": float(np.mean([t["argmax_is_perm"][t["criteria"].index(name)] for t in traces if name in t["criteria"]]))}
    inter = {f"{names[i]}~{names[j]}": spearman(fits[i], fits[j]) for i in range(len(names)) for j in range(i + 1, len(names))}
    return {"arm": arm, "calls": len(traces), "secs": sum(t["secs"] for t in traces), "input_tokens": sum(t["input_tokens"] for t in traces), "per_criterion": per, "inter_criterion": inter}


# ---------------------------------------------------------------- model

def load_model(device):
    from transformers import AutoTokenizer, DiffusionGemmaForBlockDiffusion
    from transformers.models.diffusion_gemma import modeling_diffusion_gemma as mdg
    tok = AutoTokenizer.from_pretrained(MODEL, cache_dir=CACHE)
    t0 = time.time()
    model = DiffusionGemmaForBlockDiffusion.from_pretrained(MODEL, cache_dir=CACHE, dtype=torch.bfloat16)
    print(f"loaded bf16 on cpu in {time.time()-t0:.0f}s", flush=True)
    FMAX = 448.0
    nq, cache = 0, {}   # encoder and decoder experts are tied in the checkpoint: quantize once, share
    for mod in model.modules():
        if isinstance(mod, mdg.DiffusionGemmaTextExperts):
            for pname in ("gate_up_proj", "down_proj"):
                p = getattr(mod, pname); key = p.data_ptr()
                if key not in cache:
                    w = p.data.to(device).float()
                    s = w.abs().amax(dim=-1, keepdim=True).clamp_min(1e-12) / FMAX
                    cache[key] = ((w / s).clamp(-FMAX, FMAX).to(torch.float8_e4m3fn), s.to(torch.bfloat16))
                    nq += w.numel(); del w
                delattr(mod, pname)
                setattr(mod, pname + "_q", cache[key][0]); setattr(mod, pname + "_s", cache[key][1])
    print(f"experts -> fp8: {nq/1e9:.2f}B params in {time.time()-t0:.0f}s", flush=True)

    def experts_forward(self, hidden_states, top_k_index, top_k_weights):
        final = torch.zeros_like(hidden_states)
        with torch.no_grad():
            mask = torch.nn.functional.one_hot(top_k_index, num_classes=self.num_experts).permute(2, 1, 0)
            hit = torch.greater(mask.sum(dim=(-1, -2)), 0).nonzero()
        for e in hit:
            e = e[0]
            if e == self.num_experts: continue
            top_k_pos, token_idx = torch.where(mask[e])
            x = hidden_states[token_idx]
            gu = self.gate_up_proj_q[e].to(x.dtype) * self.gate_up_proj_s[e]
            gate, up = torch.nn.functional.linear(x, gu).chunk(2, dim=-1)
            h = self.act_fn(gate) * up
            dn = self.down_proj_q[e].to(x.dtype) * self.down_proj_s[e]
            h = torch.nn.functional.linear(h, dn) * top_k_weights[token_idx, top_k_pos, None]
            final.index_add_(0, token_idx, h.to(final.dtype))
        return final
    mdg.DiffusionGemmaTextExperts.forward = experts_forward
    model.to(device).eval()
    torch.cuda.synchronize()
    print(f"on {device}: {torch.cuda.memory_allocated()/2**30:.1f} GiB in {time.time()-t0:.0f}s", flush=True)
    return tok, model


class Reader:
    def __init__(self, tok, model, k, reads, seed):
        self.tok, self.model, self.k, self.reads = tok, model, k, reads
        self.rng = random.Random(seed)
        self.vocab = model.config.text_config.vocab_size
        self.canvas_len = model.config.canvas_length
        self.turn_close = tok("<turn|>", add_special_tokens=False).input_ids
        assert len(self.turn_close) == 1
        one = lambda s: (lambda ids: ids[0] if len(ids) == 1 else None)(tok(s, add_special_tokens=False).input_ids)
        self.bare = [one(SLOTS[i]) for i in range(k)]
        self.spaced = [one(" " + SLOTS[i]) for i in range(k)]
        assert None not in self.bare and None not in self.spaced, "slot letters must be single tokens"
        self.perms = np.array(list(itertools.permutations(range(k))))

    def template(self, m):
        """Answer template ids and, per line, the k hole positions with their label token ids."""
        letters = " ".join(SLOTS[: self.k])
        text = letters if m == 1 else "\n".join(f"{i+1}: {letters}" for i in range(m))
        # Gemma 4 opens the model turn with an empty thinking channel before the answer.
        prefix = self.tok("<|channel>thought\n<channel|>", add_special_tokens=False).input_ids
        assert len(prefix) == 4, prefix
        ids = prefix + self.tok(text, add_special_tokens=False).input_ids
        holes = [(p, self.bare if t in self.bare else self.spaced) for p, t in enumerate(ids) if p >= 4 and (t in self.bare or t in self.spaced)]
        assert len(holes) == m * self.k, (len(holes), text, ids)
        for j, (p, labels) in enumerate(holes):
            assert ids[p] == labels[j % self.k]
        return text, ids, [holes[i * self.k:(i + 1) * self.k] for i in range(m)]

    @torch.no_grad()
    def read(self, system, user, m):
        text, tids, lines = self.template(m)
        msgs = [{"role": "system", "content": system}, {"role": "user", "content": user}]
        prompt = self.tok.apply_chat_template(msgs, tokenize=False, add_generation_prompt=True)
        pids = self.tok(prompt, add_special_tokens=False).input_ids
        canvas = tids + self.turn_close + [self.tok.pad_token_id] * (self.canvas_len - len(tids) - 1)
        R = self.reads
        dec = torch.tensor([canvas] * R)
        for r in range(R):
            for line in lines:
                for p, _ in line: dec[r, p] = self.rng.randrange(self.vocab)
        dev = self.model.device
        mb = max(1, min(R, 4000 // len(pids)))   # the card is shared: ~10 GiB of activation headroom
        keep = len(tids)
        chunks = []
        for s0 in range(0, R, mb):
            d = dec[s0:s0 + mb].to(dev)
            o = self.model(input_ids=torch.tensor([pids] * d.shape[0], device=dev), decoder_input_ids=d)
            chunks.append(o.logits[:, :keep, :].float()); del o; torch.cuda.empty_cache()
        logits = torch.cat(chunks, 0)
        parsed, holes_out, mats, perm_ok = [], [], [], []
        for line in lines:
            pos = [p for p, _ in line]
            lab = torch.tensor([l for _, l in line], device=dev)            # k x k label ids
            rows = logits[:, pos, :]                                        # R x k x V
            lse = torch.logsumexp(rows, dim=-1, keepdim=True)
            lab_logits = torch.gather(rows, 2, lab.unsqueeze(0).expand(R, -1, -1))   # R x k x k
            mass = torch.exp(lab_logits - lse).sum(-1)                      # R x k
            pr = torch.softmax(lab_logits, dim=-1)                          # R x k x k, per read
            mean = pr.mean(0)                                               # k x k  (rank position x slot letter)
            top = mean.argmax(-1)
            agree = (pr.argmax(-1) == top.unsqueeze(0)).float().mean(0)
            ptop = torch.gather(pr, 2, top.view(1, -1, 1).expand(R, -1, 1)).squeeze(-1)   # R x k
            se = ptop.std(0, unbiased=True) / math.sqrt(R) if R > 1 else torch.zeros_like(agree)
            ent = -(mean * torch.log(mean.clamp_min(1e-30))).sum(-1)
            ent1 = -(pr[0] * torch.log(pr[0].clamp_min(1e-30))).sum(-1)
            M = torch.log(mean.clamp_min(1e-30)).cpu().numpy()
            best = self.perms[M[np.arange(self.k), self.perms].sum(1).argmax()]
            parsed.append([int(x) for x in best])
            perm_ok.append(len(set(top.tolist())) == self.k)
            mats.append(np.round(mean.cpu().numpy(), 4).tolist())
            holes_out.append([{"label_mass": float(mass[:, j].mean()), "entropy": float(ent[j]), "entropy_first_read": float(ent1[j]), "agreement": float(agree[j]), "stderr": float(se[j]), "p_top": float(mean[j, top[j]])} for j in range(self.k)])
        return {"parsed": parsed, "holes": holes_out, "matrix": mats, "argmax_is_perm": perm_ok, "input_tokens": len(pids), "template": text}

    @torch.no_grad()
    def read_clamp(self, system, user, m):
        """Sequential clamping: encode the prompt once; at stage s the slots < s hold the letters already
        chosen, slots >= s are noise, and slot s is read as a PMF over the letters still unused. Greedy
        argmax fixes the slot. k decoder-only passes per read, R reads averaged per stage."""
        text, tids, lines = self.template(m)
        msgs = [{"role": "system", "content": system}, {"role": "user", "content": user}]
        prompt = self.tok.apply_chat_template(msgs, tokenize=False, add_generation_prompt=True)
        pids = self.tok(prompt, add_special_tokens=False).input_ids
        canvas = tids + self.turn_close + [self.tok.pad_token_id] * (self.canvas_len - len(tids) - 1)
        R, k, dev = self.reads, self.k, self.model.device
        keep = len(tids)
        enc = self.model(input_ids=torch.tensor([pids], device=dev), decoder_input_ids=torch.tensor([canvas], device=dev))
        cache = enc.past_key_values; del enc; torch.cuda.empty_cache()
        chosen = [[] for _ in lines]
        stages = [[] for _ in lines]          # per line: list of k stage PMFs (k-vectors, zeros at used letters)
        holes_out = [[] for _ in lines]
        for s in range(k):
            dec = torch.tensor([canvas] * R)
            for r in range(R):
                for li, line in enumerate(lines):
                    for j, (p, labels) in enumerate(line):
                        dec[r, p] = labels[chosen[li][j]] if j < s else self.rng.randrange(self.vocab)
            rows = []
            for r in range(R):
                o = self.model(past_key_values=cache, decoder_input_ids=dec[r:r + 1].to(dev))
                rows.append(o.logits[0, :keep, :].float()); del o
            logits = torch.stack(rows, 0)                                   # R x keep x V
            for li, line in enumerate(lines):
                p, labels = line[s]
                row = logits[:, p, :]                                       # R x V
                lse = torch.logsumexp(row, dim=-1)
                lab = torch.tensor(labels, device=dev)
                ll = row[:, lab]                                            # R x k
                mass = torch.exp(ll - lse.unsqueeze(-1)).sum(-1)            # R
                used = torch.zeros(k, dtype=torch.bool, device=dev); used[chosen[li]] = True
                ll = ll.masked_fill(used.unsqueeze(0), float("-inf"))
                pr = torch.softmax(ll, dim=-1)                              # R x k over unused letters
                mean = pr.mean(0)
                top = int(mean.argmax())
                agree = float((pr.argmax(-1) == top).float().mean())
                se = float(pr[:, top].std(unbiased=True) / math.sqrt(R)) if R > 1 else 0.0
                ent = float(-(mean * torch.log(mean.clamp_min(1e-30))).sum())
                chosen[li].append(top)
                stages[li].append(np.round(mean.cpu().numpy(), 4).tolist())
                holes_out[li].append({"label_mass": float(mass.mean()), "entropy": ent, "agreement": agree, "stderr": se, "p_top": float(mean[top]), "remaining": k - s})
        return {"parsed": [list(c) for c in chosen], "holes": holes_out, "matrix": stages, "argmax_is_perm": [True] * m, "input_tokens": len(pids), "template": text}


def run_cohort(reader, args, label):
    items = json.load(open(os.path.join(args.bench, f"{label}.json")))
    crit = json.load(open(os.path.join(args.bench, f"{label}.criteria.json")))
    names = [c["name"] for c in crit]; prompts = {c["name"]: c["prompt"] for c in crit}
    texts = []
    for it in items:
        t = " ".join(it["text"].split())
        texts.append(t[: args.max_chars] + "…" if len(t) > args.max_chars else t)
    n, m = len(texts), len(names)
    rng = random.Random(args.seed)
    plans = design(n, args.k, args.overlap, 1, args.repeats, rng)
    summaries, all_traces = [], []
    for arm in ("separate", "joint"):
        traces = []
        for pi, plan in enumerate(plans):
            for pj, pres in enumerate(plan["presentations"]):
                if arm == "separate":
                    jobs = [([nm], SYSTEM_SINGLE, prompt_single(texts, pres, prompts[nm]), 1) for nm in names]
                else:
                    order = list(range(m)); rng.shuffle(order)
                    cn = [names[i] for i in order]
                    jobs = [(cn, SYSTEM_JOINT, prompt_joint(texts, pres, [prompts[c] for c in cn]), m)]
                for cn, system, user, mm in jobs:
                    t0 = time.time()
                    res = (reader.read_clamp if args.reader in ("clamp", "ar") else reader.read)(system, user, mm)
                    res.update({"arm": arm, "plan": pi, "presentation": pj, "order": pres, "criteria": cn, "secs": time.time() - t0})
                    traces.append(res)
            print(f"[{label}/{arm}] window {pi+1}/{len(plans)}  {traces[-1]['secs']:.1f}s/read-set  tokens={traces[-1]['input_tokens']}", flush=True)
        summaries.append(summarize(arm, names, n, traces, plans)); all_traces += traces
    sep, joi = summaries
    agreement = {nm: {"rho": spearman(sep["per_criterion"][nm]["scores"], joi["per_criterion"][nm]["scores"]), "top10": topk_overlap(sep["per_criterion"][nm]["scores"], joi["per_criterion"][nm]["scores"], 10)} for nm in names}
    halo = float(np.mean(list(joi["inter_criterion"].values())) - np.mean(list(sep["inter_criterion"].values())))
    vs = {}
    base = os.path.join(args.baselines, f"summary-{label}.json")
    if os.path.exists(base):
        b = json.load(open(base)); assert b["ids"] == [it["id"] for it in items]
        for arm_s in b["arms"]:
            for nm in names:
                for mine in summaries:
                    vs[f"{nm}: dg_{mine['arm']} ~ gemma31b_{arm_s['arm']}"] = spearman(mine["per_criterion"][nm]["scores"], arm_s["per_criterion"][nm]["scores"])
    out = {"label": label, "model": MODEL, "expert_quant": QUANT, "reader": args.reader, "reads": args.reads, "k": args.k, "overlap": args.overlap, "repeats": args.repeats, "seed": args.seed,
           "ids": [it["id"] for it in items], "criteria": [[c["name"], c["prompt"]] for c in crit], "arms": summaries, "agreement": agreement, "halo_inflation": halo, "vs_gemma31b": vs}
    os.makedirs(args.out, exist_ok=True)
    json.dump(out, open(os.path.join(args.out, f"summary-{label}.json"), "w"), indent=1)
    with open(os.path.join(args.out, f"trace-{label}.jsonl"), "w") as f:
        for t in all_traces: f.write(json.dumps(t) + "\n")
    print(f"== {label}: agreement " + " / ".join(f"{agreement[nm]['rho']:+.3f}" for nm in names) + f" | halo {halo:+.3f} | flip sep " + "/".join(f"{sep['per_criterion'][nm]['flip']:.3f}" for nm in names) + " -> joint " + "/".join(f"{joi['per_criterion'][nm]['flip']:.3f}" for nm in names), flush=True)
    for k_, v in vs.items(): print(f"   {k_}: {v:+.3f}", flush=True)


def smoke(reader):
    tok, model = reader.tok, reader.model
    texts = ["The number 3.", "The number 9000.", "The number 40.", "The number 0.5.", "The number 700.", "The number 12.", "The number 100000.", "The number 1."]
    order = list(range(8))
    user = prompt_single(texts, order, "Magnitude: how large the number is.")
    for msgs in ([{"role": "user", "content": "Name the three primary colours in one short sentence."}], [{"role": "system", "content": SYSTEM_SINGLE}, {"role": "user", "content": user}]):
        enc = tok.apply_chat_template(msgs, add_generation_prompt=True, return_tensors="pt", return_dict=True).to(model.device)
        t0 = time.time(); g = model.generate(**enc, max_new_tokens=256)
        print("GEN", f"{time.time()-t0:.1f}s", "in", enc["input_ids"].shape, "keys", [k for k in g.keys()] if hasattr(g, "keys") else type(g), repr(tok.decode(getattr(g, "sequences", g)[0][-256:], skip_special_tokens=False)[:300]), flush=True)
    # what does one denoiser forward want to write over the seeded canvas?
    text, tids, lines = reader.template(1)
    prompt = tok.apply_chat_template([{"role": "system", "content": SYSTEM_SINGLE}, {"role": "user", "content": user}], tokenize=False, add_generation_prompt=True)
    print("PROMPT TAIL", repr(prompt[-120:]), flush=True)
    pids = tok(prompt, add_special_tokens=False).input_ids
    canvas = tids + reader.turn_close + [tok.pad_token_id] * (reader.canvas_len - len(tids) - 1)
    for variant in ("seeded", "all-noise"):
        dec = torch.tensor([canvas])
        if variant == "all-noise": dec = torch.randint(0, reader.vocab, dec.shape)
        else:
            for p, _ in lines[0]: dec[0, p] = random.randrange(reader.vocab)
        with torch.no_grad():
            lg = model(input_ids=torch.tensor([pids], device=model.device), decoder_input_ids=dec.to(model.device)).logits[0]
        am = lg.argmax(-1)[:24].tolist()
        print("FWD", variant, "in", [tok.decode([t]) for t in dec[0, :18].tolist()], "->", [tok.decode([t]) for t in am], flush=True)
    r = reader.read(SYSTEM_SINGLE, user, 1)
    print("READ", [texts[order[s]] for s in r["parsed"][0]], "mass", [round(h["label_mass"], 3) for h in r["holes"][0]], flush=True)
    t0 = time.time(); r = reader.read_clamp(SYSTEM_SINGLE, user, 1)
    print(f"CLAMP {time.time()-t0:.1f}s", [texts[order[s]] for s in r["parsed"][0]], "p_top", [round(h["p_top"], 2) for h in r["holes"][0]], "mass", [round(h["label_mass"], 2) for h in r["holes"][0]], flush=True)


def smoke_masked(reader, args):
    tok, model = reader.tok, reader.model
    texts = ["The number 3.", "The number 9000.", "The number 40.", "The number 0.5.", "The number 700.", "The number 12.", "The number 100000.", "The number 1."]
    order = list(range(8))
    user = prompt_single(texts, order, "Magnitude: how large the number is.")
    pids = reader.prompt_ids(SYSTEM_SINGLE, user)
    print("PROMPT TAIL", repr(tok.decode(pids[-40:])[-100:]), flush=True)
    ids = torch.tensor([pids], device=model.device)
    with torch.no_grad():
        reader.prefill(pids)
        p0 = torch.softmax(reader.block_logits([reader.mask_id] * 15)[0], -1); top = torch.topk(p0, 6)
        print("SLOT1 TOP", [(tok.decode([int(i)]), round(float(v), 3)) for v, i in zip(top.values, top.indices)], flush=True)
    t0 = time.time()
    with torch.no_grad():
        if args.model.startswith("nemotron"):
            g, nfe = model.generate(ids, max_new_tokens=32, block_length=32, threshold=0.9, eos_token_id=tok.eos_token_id)
        elif args.model.startswith("sdar") or args.model == "dream7":
            eos = tok.convert_tokens_to_ids("<|im_end|>")
            g = torch.tensor([reader.greedy_fill(ids[0].tolist(), 32, reader.bl if hasattr(reader, "bl") else 32, eos)], device=ids.device)
        else:
            g = model.generate(inputs=ids, gen_length=32, block_length=32, steps=32)
    new = g[0, ids.shape[1]:ids.shape[1]+32] if g.shape[1] > ids.shape[1] else g[0, :32]
    print("GEN", f"{time.time()-t0:.1f}s", repr(tok.decode(new.tolist())), flush=True)
    text, tids, lines = reader.template(1)
    print("TEMPLATE", [tok.decode([t]) for t in tids], "mask", reader.mask_id, flush=True)
    t0 = time.time(); r = reader.read(SYSTEM_SINGLE, user, 1)
    print(f"READ {time.time()-t0:.2f}s", [texts[order[s]] for s in r["parsed"][0]], "p_top", [round(h["p_top"], 2) for h in r["holes"][0]], "mass", [round(h["label_mass"], 2) for h in r["holes"][0]], flush=True)
    t0 = time.time(); r = reader.read_clamp(SYSTEM_SINGLE, user, 1)
    print(f"CLAMP {time.time()-t0:.2f}s", [texts[order[s]] for s in r["parsed"][0]], "p_top", [round(h["p_top"], 2) for h in r["holes"][0]], "mass", [round(h["label_mass"], 2) for h in r["holes"][0]], flush=True)
    t0 = time.time(); r = reader.read_clamp(SYSTEM_JOINT, prompt_joint(texts, order, ["Magnitude: how large the number is.", "Smallness: how small the number is.", "Closeness to 10."]), 3)
    print(f"CLAMP-JOINT {time.time()-t0:.2f}s", [[texts[order[s]][11:-1] for s in line] for line in r["parsed"]], flush=True)


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["smoke", "bench"])
    ap.add_argument("--cohorts", default="hn_top")
    ap.add_argument("--bench", default="bench"); ap.add_argument("--baselines", default="baselines"); ap.add_argument("--out", default="out")
    ap.add_argument("--k", type=int, default=8); ap.add_argument("--overlap", type=int, default=2); ap.add_argument("--repeats", type=int, default=2)
    ap.add_argument("--model", default="dg"); ap.add_argument("--lora", default=None); ap.add_argument("--reader", choices=["one", "clamp", "ar"], default="one"); ap.add_argument("--reads", type=int, default=4); ap.add_argument("--seed", type=int, default=7); ap.add_argument("--max-chars", type=int, default=3000)
    args = ap.parse_args()
    if args.model == "dg":
        tok, model = load_model("cuda:0")
        if args.lora:
            import dg_lora
            dg_lora.load(dg_lora.apply(model), args.lora); MODEL = MODEL + " + lora " + os.path.basename(args.lora)
        reader = Reader(tok, model, args.k, args.reads, args.seed)
        smoke(reader)
    else:
        import dlm_readers as dr
        if args.model.startswith("nemotron"):
            name = {"nemotron14": "nvidia/Nemotron-Labs-Diffusion-14B", "nemotron8": "nvidia/Nemotron-Labs-Diffusion-8B"}[args.model]
            tok, model = dr.load_nemotron(name, "cuda:0")
            reader = dr.NemotronReader(tok, model, args.k, args.reads, args.seed, ar=(args.reader == "ar"))
        elif args.model.startswith("sdar"):
            name = {"sdar8": "JetLM/SDAR-8B-Chat", "sdar30": "JetLM/SDAR-30B-A3B-Chat"}[args.model]
            tok, model = dr.load_sdar(name, "cuda:0")
            reader = dr.SDARReader(tok, model, args.k, args.reads, args.seed)
        elif args.model == "dream7":
            name = "Dream-org/Dream-v0-Instruct-7B"
            tok, model = dr.load_dream(name, "cuda:0")
            reader = dr.DreamReader(tok, model, args.k, args.reads, args.seed)
        else:
            name = {"llada22mini": "inclusionAI/LLaDA2.2-mini"}[args.model]
            tok, model = dr.load_llada2(name, "cuda:0")
            reader = dr.LLaDA2Reader(tok, model, args.k, args.reads, args.seed)
        MODEL = name + ("" if args.reader != "ar" else " (AR)"); QUANT = "bf16"
        smoke_masked(reader, args)
    if args.mode == "bench":
        for label in args.cohorts.split(","): run_cohort(reader, args, label)
