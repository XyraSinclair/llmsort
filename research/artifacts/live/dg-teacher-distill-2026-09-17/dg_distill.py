"""Teacher distillation: DiffusionGemma's one-forward slot read <- gemma-4-31b's setwise latents.

Teacher: research/artifacts/live/mc-teacher-corpus-2026-09-13 (1,000 LessWrong lists x 3 criteria, per-item
Plackett-Luce latents fitted from gemma-4-31b joint setwise calls). A training example is a random 8-subset of a
list under one of its criteria, in a random letter order, prompted exactly as the bench's separate arm; the target
is the exact PL rank-marginal matrix (slot x letter) implied by the teacher latents; the loss is the per-slot
cross-entropy of the student's one-forward letter PMF (holes filled with uniform-random tokens, as at read time)
against that matrix. Encoder pass without grad (its KV cache is read-only for the decoder); LoRA on the decoder.
Lists containing any bench lw item are excluded; 24 further lists are held out for the validation CE.
"""
import argparse, json, math, os, random, time
import numpy as np, torch
import dg_setwise as B, dg_lora

CORPUS = "/srv/build/llmsort-bakeoff/research/artifacts/live/mc-teacher-corpus-2026-09-13"


def pl_marginals(theta):
    """Exact Plackett-Luce rank marginals M[s, j] = P(item j at rank s) by DP over chosen subsets (2^k states)."""
    k = len(theta); w = np.exp(theta - theta.max()); M = np.zeros((k, k)); f = np.zeros(1 << k); f[0] = 1.0
    for S in range(1 << k):
        if f[S] == 0: continue
        s = bin(S).count("1")
        if s == k: continue
        rem = [j for j in range(k) if not S >> j & 1]; z = sum(w[j] for j in rem)
        for j in rem:
            p = f[S] * w[j] / z; M[s, j] += p; f[S | 1 << j] += p
    return M


def load_corpus(bench_lw, holdout_lists, seed):
    cohorts = json.load(open(os.path.join(CORPUS, "cohorts.json")))
    latents = {}
    for line in open(os.path.join(CORPUS, "ledger.jsonl")):
        r = json.loads(line); latents.setdefault(r["list_id"], {}).setdefault(r["criterion_name"], {})[r["item_id"]] = r["latent"]
    bench_ids = {it["id"] for it in json.load(open(bench_lw))}
    lists = []
    for c in cohorts:
        items = json.load(open(os.path.join(CORPUS, "lists", c["shard"], f"{c['list_id']}.json")))
        if any(it["id"] in bench_ids for it in items): continue
        if c["list_id"] not in latents: continue
        texts = []
        for it in items:
            t = " ".join(it["text"].split()); texts.append(t[:3000] + "…" if len(t) > 3000 else t)
        crit = [(cc["name"], cc["text"], np.array([latents[c["list_id"]][cc["name"]][it["id"]] for it in items])) for cc in c["criteria"] if cc["name"] in latents[c["list_id"]]]
        if len(crit) < 3: continue
        lists.append({"id": c["list_id"], "texts": texts, "criteria": crit, "source": c["criteria_source"]})
    rng = random.Random(seed); rng.shuffle(lists)
    return lists[holdout_lists:], lists[:holdout_lists], len(cohorts) - len(lists)


def make_example(lst, rng, k):
    name, text, lat = lst["criteria"][rng.randrange(len(lst["criteria"]))]
    order = rng.sample(range(len(lst["texts"])), k)
    return {"list": lst["id"], "criterion": name, "order": order, "user": B.prompt_single(lst["texts"], order, text), "target": pl_marginals(lat[order])}


def enable_checkpointing(model):
    """Per-layer activation checkpointing on the decoder that keeps the read-only encoder cache (the stock
    GradientCheckpointingLayer path strips past_key_values, and the model class refuses to enable it)."""
    from torch.utils.checkpoint import checkpoint
    for layer in model.model.decoder.layers:
        orig = layer.forward
        def fwd(hidden_states, *a, _orig=orig, **kw):
            if not torch.is_grad_enabled(): return _orig(hidden_states, *a, **kw)
            return checkpoint(lambda h: _orig(h, *a, **kw), hidden_states, use_reentrant=False)
        layer.forward = fwd


class Student:
    def __init__(self, reader):
        self.r = reader; self.tok, self.model = reader.tok, reader.model
        self.text, self.tids, self.lines = reader.template(1)
        self.canvas = self.tids + reader.turn_close + [self.tok.pad_token_id] * (reader.canvas_len - len(self.tids) - 1)

    def forward(self, user, rng, grad):
        """One noise draw: returns k x k log-PMF over letters at the k rank positions."""
        dev = self.model.device
        msgs = [{"role": "system", "content": B.SYSTEM_SINGLE}, {"role": "user", "content": user}]
        pids = self.tok(self.tok.apply_chat_template(msgs, tokenize=False, add_generation_prompt=True), add_special_tokens=False).input_ids
        dec = list(self.canvas)
        for p, _ in self.lines[0]: dec[p] = rng.randrange(self.r.vocab)
        dec = torch.tensor([dec], device=dev)
        with torch.no_grad():
            enc = self.model(input_ids=torch.tensor([pids], device=dev), decoder_input_ids=dec)
            cache = enc.past_key_values; del enc
        with torch.set_grad_enabled(grad):
            out = self.model(past_key_values=cache, decoder_input_ids=dec)
            pos = [p for p, _ in self.lines[0]]; lab = torch.tensor([l for _, l in self.lines[0]], device=dev)
            rows = out.logits[0, pos, :].float()                       # k x V
            ll = torch.gather(rows, 1, lab)                            # k x k
            logq = ll - torch.logsumexp(rows, -1, keepdim=True)          # log-prob of each letter under the FULL vocab softmax
            mass = torch.exp(torch.logsumexp(logq, -1)).mean()
        del cache, out, rows
        return logq, float(mass), len(pids)


def ce(logq, target):
    t = torch.tensor(target, device=logq.device, dtype=torch.float32)
    return -(t * logq).sum(-1).mean()


def mem(tag):
    print(f"[mem] {tag}: alloc {torch.cuda.memory_allocated()/2**30:.2f}G  peak {torch.cuda.max_memory_allocated()/2**30:.2f}G", flush=True)


def evaluate(student, examples, seed):
    rng = random.Random(seed); tot = 0.0; agree = 0.0
    if not examples: return float("nan"), float("nan")
    for ex in examples:
        logq, mass, _ = student.forward(ex["user"], rng, grad=False)
        tot += float(ce(logq - torch.logsumexp(logq, -1, keepdim=True), ex["target"]))
        agree += float((logq.argmax(-1).cpu().numpy() == ex["target"].argmax(-1)).mean())
    return tot / len(examples), agree / len(examples)


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="lora"); ap.add_argument("--examples", type=int, default=4000); ap.add_argument("--accum", type=int, default=4)
    ap.add_argument("--lr", type=float, default=2e-4); ap.add_argument("--rank", type=int, default=16); ap.add_argument("--alpha", type=float, default=32)
    ap.add_argument("--eval-every", type=int, default=400); ap.add_argument("--eval-n", type=int, default=48); ap.add_argument("--holdout-lists", type=int, default=24)
    ap.add_argument("--seed", type=int, default=11); ap.add_argument("--k", type=int, default=8); ap.add_argument("--resume", default=None); ap.add_argument("--max-tokens", type=int, default=5600)
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    torch.manual_seed(args.seed)
    train, held, dropped = load_corpus("bench/lw.json", args.holdout_lists, args.seed)
    print(f"corpus: {len(train)} train lists, {len(held)} held-out lists, {dropped} dropped (bench-lw overlap or unfitted)", flush=True)
    rng = random.Random(args.seed); erng = random.Random(args.seed + 1)
    evalset = [make_example(held[i % len(held)], erng, args.k) for i in range(args.eval_n)]
    tok, model = B.load_model("cuda:0")
    model.config.text_config.attention_dropout = 0.0
    wrapped = dg_lora.apply(model, args.rank, args.alpha)
    if args.resume: dg_lora.load(wrapped, args.resume)
    params = dg_lora.parameters(wrapped)
    print(f"lora: {len(wrapped)} modules, {sum(p.numel() for p in params)/1e6:.1f}M params", flush=True)
    for p in model.parameters(): p.requires_grad_(False)
    for p in params: p.requires_grad_(True)
    enable_checkpointing(model); model.eval()
    reader = B.Reader(tok, model, args.k, 1, args.seed); student = Student(reader)
    opt = torch.optim.AdamW(params, lr=args.lr, weight_decay=0.0, betas=(0.9, 0.99))
    steps = args.examples // args.accum; warm = max(1, steps // 20)
    sched = torch.optim.lr_scheduler.LambdaLR(opt, lambda s: min(1.0, (s + 1) / warm) * 0.5 * (1 + math.cos(math.pi * min(1.0, s / steps))))
    log = open(os.path.join(args.out, "train.jsonl"), "a")
    mem("after load+lora")
    v_ce, v_agree = evaluate(student, evalset, 99); torch.cuda.empty_cache(); mem("after eval@0")
    tent = float(np.mean([-(e["target"] * np.log(np.maximum(e["target"], 1e-12))).sum(-1).mean() for e in evalset]))
    print(f"eval@0: ce {v_ce:.4f} top1 {v_agree:.3f} (uniform ce {math.log(args.k):.4f}, target entropy {tent:.4f}; eval ce is over the letters, train ce over the full vocab)", flush=True)
    log.write(json.dumps({"examples": 0, "eval_ce": v_ce, "eval_top1": v_agree, "t": time.time()}) + "\n"); log.flush()
    t0 = time.time(); run_ce, run_mass, run_tok = [], [], []; skipped = 0
    for i in range(1, args.examples + 1):
        ex = make_example(train[rng.randrange(len(train))], rng, args.k)
        if i <= 3: mem(f"before example {i}")
        if len(student.tok(ex["user"]).input_ids) > args.max_tokens: skipped += 1; continue
        logq, mass, ntok = student.forward(ex["user"], rng, grad=True)
        if i <= 3: mem(f"after forward {i} ({ntok} tokens)")
        loss = ce(logq, ex["target"]) / args.accum
        loss.backward(); lv = float(loss) * args.accum; del logq, loss
        if i <= 3: mem(f"after backward {i}")
        run_ce.append(lv); run_mass.append(mass); run_tok.append(ntok)
        if i % args.accum == 0:
            torch.nn.utils.clip_grad_norm_(params, 1.0); opt.step(); sched.step(); opt.zero_grad(set_to_none=True)
        if i % 20 == 0:
            el = time.time() - t0
            print(f"ex {i}/{args.examples}  ce {np.mean(run_ce[-20:]):.4f}  mass {np.mean(run_mass[-20:]):.3f}  tok {np.mean(run_tok[-20:]):.0f}  {el/i:.2f}s/ex  eta {(args.examples-i)*el/i/60:.0f}m  skipped {skipped}  mem {torch.cuda.max_memory_allocated()/2**30:.1f}G", flush=True)
            log.write(json.dumps({"examples": i, "ce": float(np.mean(run_ce[-20:])), "mass": float(np.mean(run_mass[-20:])), "lr": sched.get_last_lr()[0], "t": time.time()}) + "\n"); log.flush()
        if i % args.eval_every == 0 or i == args.examples:
            v_ce, v_agree = evaluate(student, evalset, 99)
            torch.save(dg_lora.state(wrapped), os.path.join(args.out, f"step-{i}.pt")); torch.save(dg_lora.state(wrapped), os.path.join(args.out, "latest.pt"))
            print(f"eval@{i}: ce {v_ce:.4f} top1 {v_agree:.3f}  saved", flush=True)
            log.write(json.dumps({"examples": i, "eval_ce": v_ce, "eval_top1": v_agree, "t": time.time()}) + "\n"); log.flush()
