"""Pointwise scorer on the four bench cohorts: per criterion, Spearman to gemma-4-31b's separate-arm scores; the
inter-criterion structure (student vs gemma); items/s. Flips are zero by construction (no presentation)."""
import argparse, json, os, sys, time, numpy as np, torch
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from ft_scorer import Scorer, apply_lora, spearman
ap = argparse.ArgumentParser()
ap.add_argument("--model", default="Qwen/Qwen3-Reranker-0.6B"); ap.add_argument("--weights", default=None); ap.add_argument("--full", action="store_true")
ap.add_argument("--rank", type=int, default=16); ap.add_argument("--alpha", type=float, default=32)
ap.add_argument("--bench", default="bench"); ap.add_argument("--baselines", default="baselines"); ap.add_argument("--cohorts", default="hn_top,hn_comments,arxiv,lw")
ap.add_argument("--max-chars", type=int, default=3000); ap.add_argument("--max-tokens", type=int, default=1024); ap.add_argument("--chunk", type=int, default=40); ap.add_argument("--out", required=True)
args = ap.parse_args()
sc = Scorer(args.model, args.max_tokens, "cuda:0"); sc.model.eval()
if args.weights:
    state = torch.load(args.weights)
    if args.full: sc.model.load_state_dict(state)
    else:
        wrapped = apply_lora(sc.model, args.rank, args.alpha); assert set(wrapped) == set(state), "adapter key mismatch"
        for k, w in wrapped.items(): w.A.data.copy_(state[k]["A"].to(w.A.device)); w.B.data.copy_(state[k]["B"].to(w.B.device))
out = {"model": args.model, "weights": args.weights, "cohorts": {}}
with torch.no_grad():
    for label in args.cohorts.split(","):
        items = json.load(open(f"{args.bench}/{label}.json")); crit = json.load(open(f"{args.bench}/{label}.criteria.json"))
        texts = [" ".join(it["text"].split())[:args.max_chars] for it in items]
        base = json.load(open(f"{args.baselines}/summary-{label}.json")); gsep = [a for a in base["arms"] if a["arm"] == "separate"][0]["per_criterion"]
        t0 = time.time(); s = {c["name"]: sc.scores(c["prompt"], texts, args.chunk).cpu().numpy() for c in crit}; dt = time.time() - t0
        names = [c["name"] for c in crit]
        rho = {nm: spearman(s[nm], np.array(gsep[nm]["scores"])) for nm in names}
        inter_s = [spearman(s[a], s[b]) for i, a in enumerate(names) for b in names[i + 1:]]
        inter_g = [spearman(np.array(gsep[a]["scores"]), np.array(gsep[b]["scores"])) for i, a in enumerate(names) for b in names[i + 1:]]
        out["cohorts"][label] = {"rho_gemma": rho, "inter_student": inter_s, "inter_gemma": inter_g, "seconds": dt, "items_per_s": len(texts) * len(names) / dt, "scores": {k: v.tolist() for k, v in s.items()}}
        print(f"== {label}: rho~gemma " + " / ".join(f"{rho[nm]:+.3f}" for nm in names) + f" | inter-criterion student {np.mean(inter_s):+.3f} vs gemma {np.mean(inter_g):+.3f} | {len(texts)*len(names)/dt:.0f} items/s", flush=True)
json.dump(out, open(args.out, "w"), indent=1)
