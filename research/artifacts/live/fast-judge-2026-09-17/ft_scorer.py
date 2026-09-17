"""Criterion-conditioned pointwise scorer distilled from gemma-4-31b's Plackett-Luce latents.

Student: a Qwen3 causal LM (Qwen3-Reranker-0.6B by default) in the reranker prompt format — the criterion text is
the query, the item text the document — scored as logit(yes) - logit(no) at the last position. One training step is
one (list, criterion): all 40 items scored in one batch, loss = soft RankNet over every pair,
BCE(sigmoid(s_i - s_j), sigmoid(theta_i - theta_j)), theta the teacher latents (ln 2.78 per pair units).
Eval: the fixed eval99 lists (33 per criteria source, seed 2026 — the same lists zs_judge.py scores): per
(list, criterion) Spearman to the teacher, by source, plus the inter-criterion structure match. Train lists are
the rest minus any list sharing an item with the bench lw cohort."""
import argparse, collections, json, math, os, random, sys, time
import numpy as np, torch, torch.nn as nn, torch.nn.functional as F
from transformers import AutoModelForCausalLM, AutoTokenizer
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from dg_lora import LoRALinear
C = "/srv/build/llmsort-bakeoff/research/artifacts/live/mc-teacher-corpus-2026-09-13"
PREFIX = '<|im_start|>system\nJudge whether the Document meets the requirements based on the Query and the Instruct provided. Note that the answer can only be "yes" or "no".<|im_end|>\n<|im_start|>user\n'
SUFFIX = '<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n'
INSTRUCT = "Judge how strongly the Document exhibits the attribute described in the Query, relative to other documents of its kind."

def eval_lists(coh, per=33):
    by = collections.defaultdict(list)
    for c in coh: by[c["criteria_source"]].append(c)
    rng = random.Random(2026); sel = []
    for s in ("lw", "highdim_elaborated", "fable_subtle_1000_elaborated"): sel += rng.sample(by[s], 33)[:per]
    return sel

def spearman(a, b):
    ra = np.argsort(np.argsort(a)); rb = np.argsort(np.argsort(b)); return float(np.corrcoef(ra, rb)[0, 1])

def load_corpus(bench_lw, max_chars):
    coh = json.load(open(C + "/cohorts.json")); ev = {c["list_id"] for c in eval_lists(coh)}
    bench_ids = {it["id"] for it in json.load(open(bench_lw))}
    lat = collections.defaultdict(dict)
    for line in open(C + "/ledger.jsonl"):
        r = json.loads(line); lat[(r["list_id"], r["criterion_name"])][r["item_id"]] = r["latent"]
    train, evl, dropped = [], [], 0
    for c in coh:
        items = json.load(open(f"{C}/lists/{c['shard']}/{c['list_id']}.json"))
        ids = [it["id"] for it in items]; texts = [it["text"][:max_chars] for it in items]
        crit = [(cc["name"], cc["text"], np.array([lat[(c["list_id"], cc["name"])][i] for i in ids])) for cc in c["criteria"] if all(i in lat[(c["list_id"], cc["name"])] for i in ids)]
        rec = {"id": c["list_id"], "source": c["criteria_source"], "texts": texts, "criteria": crit}
        if c["list_id"] in ev: evl.append(rec)
        elif any(i in bench_ids for i in ids) or len(crit) < 3: dropped += 1
        else: train.append(rec)
    return train, evl, dropped

class Scorer:
    def __init__(self, model_id, max_tokens, device):
        self.tok = AutoTokenizer.from_pretrained(model_id); self.tok.padding_side = "right"
        self.model = AutoModelForCausalLM.from_pretrained(model_id, dtype=torch.bfloat16).to(device)
        self.yes, self.no = self.tok.convert_tokens_to_ids("yes"), self.tok.convert_tokens_to_ids("no")
        self.pre = self.tok(PREFIX, add_special_tokens=False)["input_ids"]; self.suf = self.tok(SUFFIX, add_special_tokens=False)["input_ids"]
        self.max_tokens, self.device = max_tokens, device
    def encode(self, criterion, texts):
        head = self.tok(f"<Instruct>: {INSTRUCT}\n<Query>: {criterion}\n<Document>: ", add_special_tokens=False)["input_ids"]
        room = self.max_tokens - len(self.pre) - len(head) - len(self.suf)
        assert room > 64, "criterion too long for max_tokens"
        seqs = [self.pre + head + self.tok(t, add_special_tokens=False)["input_ids"][:room] + self.suf for t in texts]
        L = max(len(s) for s in seqs); pad = self.tok.pad_token_id
        ids = torch.tensor([s + [pad] * (L - len(s)) for s in seqs], device=self.device)
        att = torch.tensor([[1] * len(s) + [0] * (L - len(s)) for s in seqs], device=self.device)
        last = torch.tensor([len(s) - 1 for s in seqs], device=self.device)
        return ids, att, last
    def scores(self, criterion, texts, chunk):
        out = []
        for i in range(0, len(texts), chunk):
            ids, att, last = self.encode(criterion, texts[i:i + chunk])
            h = self.model.model(input_ids=ids, attention_mask=att).last_hidden_state  # lm_head only at the last position
            row = self.model.lm_head(h[torch.arange(ids.shape[0], device=self.device), last])
            out.append((row[:, self.yes] - row[:, self.no]).float())
        return torch.cat(out)

def apply_lora(model, r, alpha, targets=("q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj")):
    wrapped = {}
    for name, mod in list(model.named_modules()):
        for attr, child in list(mod.named_children()):
            if attr in targets and isinstance(child, nn.Linear):
                lora = LoRALinear(child, r, alpha).to(child.weight.device); setattr(mod, attr, lora); wrapped[f"{name}.{attr}"] = lora
    return wrapped

def ranknet(s, theta):
    ds = s[:, None] - s[None, :]; dt = theta[:, None] - theta[None, :]
    iu = torch.triu_indices(len(s), len(s), 1, device=s.device)
    return F.binary_cross_entropy_with_logits(ds[iu[0], iu[1]], torch.sigmoid(dt[iu[0], iu[1]]))

@torch.no_grad()
def evaluate(sc, evl, chunk):
    sc.model.eval(); rows, struct = [], []; t0 = time.time(); n_items = 0
    for lst in evl:
        got = {}
        for name, text, theta in lst["criteria"]:
            s = sc.scores(text, lst["texts"], chunk).cpu().numpy(); got[name] = (s, theta); n_items += len(s)
            rows.append({"list": lst["id"], "criterion": name, "source": lst["source"], "rho": spearman(s, theta)})
        names = list(got)
        for i in range(len(names)):
            for j in range(i + 1, len(names)):
                struct.append({"list": lst["id"], "source": lst["source"], "pair": [names[i], names[j]],
                               "judge": spearman(got[names[i]][0], got[names[j]][0]), "teacher": spearman(got[names[i]][1], got[names[j]][1])})
    dt = time.time() - t0
    summ = {"rho_mean": float(np.mean([r["rho"] for r in rows])),
            "rho_by_source": {s: float(np.mean([r["rho"] for r in rows if r["source"] == s])) for s in sorted({r["source"] for r in rows})},
            "struct_absdiff_mean": float(np.mean([abs(x["judge"] - x["teacher"]) for x in struct])),
            "struct_corr": float(np.corrcoef([x["judge"] for x in struct], [x["teacher"] for x in struct])[0, 1]),
            "items_per_s": n_items / dt, "seconds": dt}
    return summ, rows, struct

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", default="Qwen/Qwen3-Reranker-0.6B"); ap.add_argument("--out", default="scorer")
    ap.add_argument("--epochs", type=float, default=1.0); ap.add_argument("--lr", type=float, default=1e-4)
    ap.add_argument("--rank", type=int, default=16); ap.add_argument("--alpha", type=float, default=32); ap.add_argument("--full", action="store_true")
    ap.add_argument("--eval-every", type=int, default=300); ap.add_argument("--eval-lists", type=int, default=99)
    ap.add_argument("--max-chars", type=int, default=3000); ap.add_argument("--max-tokens", type=int, default=1024)
    ap.add_argument("--chunk", type=int, default=40); ap.add_argument("--seed", type=int, default=11); ap.add_argument("--steps", type=int, default=0)
    ap.add_argument("--checkpointing", action="store_true")
    args = ap.parse_args(); os.makedirs(args.out, exist_ok=True)
    torch.manual_seed(args.seed); rng = random.Random(args.seed)
    train, evl, dropped = load_corpus("bench/lw.json", args.max_chars); evl = evl[:args.eval_lists]
    print(f"corpus: {len(train)} train lists, {len(evl)} eval lists, {dropped} dropped", flush=True)
    sc = Scorer(args.model, args.max_tokens, "cuda:0")
    if args.checkpointing: sc.model.gradient_checkpointing_enable(gradient_checkpointing_kwargs={"use_reentrant": False})
    if args.full: params = list(sc.model.parameters())
    else:
        for p in sc.model.parameters(): p.requires_grad_(False)
        wrapped = apply_lora(sc.model, args.rank, args.alpha); params = [p for w in wrapped.values() for p in (w.A, w.B)]
        print(f"lora: {len(wrapped)} modules, {sum(p.numel() for p in params)/1e6:.1f}M params", flush=True)
    steps = [(l, ci) for l in train for ci in range(len(l["criteria"]))]
    total = args.steps or int(len(steps) * args.epochs)
    opt = torch.optim.AdamW(params, lr=args.lr, betas=(0.9, 0.99), weight_decay=0.0); warm = max(1, total // 20)
    sched = torch.optim.lr_scheduler.LambdaLR(opt, lambda i: min(1.0, (i + 1) / warm) * 0.5 * (1 + math.cos(math.pi * min(i, total) / total)))
    log = open(os.path.join(args.out, "train.jsonl"), "a")
    def do_eval(step):
        summ, rows, struct = evaluate(sc, evl, args.chunk); sc.model.train()
        print(f"eval@{step}: rho {summ['rho_mean']:.3f} by-source {json.dumps({k: round(v, 3) for k, v in summ['rho_by_source'].items()})} struct-absdiff {summ['struct_absdiff_mean']:.3f} struct-corr {summ['struct_corr']:.3f} {summ['items_per_s']:.1f} items/s", flush=True)
        log.write(json.dumps({"step": step, "eval": summ}) + "\n"); log.flush()
        json.dump({"summary": summ, "rows": rows, "struct": struct}, open(os.path.join(args.out, f"eval-{step}.json"), "w"), indent=1)
        state = {k: v.detach().cpu() for k, v in sc.model.state_dict().items()} if args.full else {k: {"A": w.A.detach().cpu(), "B": w.B.detach().cpu()} for k, w in wrapped.items()}
        torch.save(state, os.path.join(args.out, "latest.pt"))
    do_eval(0); sc.model.train(); t0 = time.time(); acc_loss = acc_rho = 0.0
    order = list(range(len(steps)))
    for i in range(total):
        if i % len(steps) == 0: rng.shuffle(order)
        lst, ci = steps[order[i % len(steps)]]; name, text, theta = lst["criteria"][ci]
        s = sc.scores(text, lst["texts"], args.chunk); th = torch.tensor(theta, device=s.device, dtype=torch.float32)
        loss = ranknet(s, th); loss.backward()
        torch.nn.utils.clip_grad_norm_(params, 1.0); opt.step(); sched.step(); opt.zero_grad(set_to_none=True)
        acc_loss += float(loss); acc_rho += spearman(s.detach().cpu().numpy(), theta)
        if (i + 1) % 20 == 0:
            el = time.time() - t0
            print(f"step {i+1}/{total}  loss {acc_loss/20:.4f}  rho {acc_rho/20:.3f}  {el/(i+1):.2f}s/step  eta {el/(i+1)*(total-i-1)/60:.0f}m  mem {torch.cuda.max_memory_allocated()/2**30:.1f}G", flush=True)
            log.write(json.dumps({"step": i + 1, "loss": acc_loss / 20, "rho": acc_rho / 20, "t": time.time()}) + "\n"); log.flush(); acc_loss = acc_rho = 0.0
        if (i + 1) % args.eval_every == 0 and i + 1 < total: do_eval(i + 1)
    do_eval(total)

if __name__ == "__main__": main()
