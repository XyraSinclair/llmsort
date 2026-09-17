"""Teacher ceiling: split-half (presentation 0 vs 1) reliability of gemma-4-31b and of DG+LoRA on the bench cohorts
with a gemma trace, Spearman-Brown corrected to the full 2-presentation design, and the disattenuated
student-teacher correlation rho_obs / sqrt(r_teacher * r_student)."""
import json, os, sys, numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from soft_refit import summarize, spearman
G = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "multi-criteria-setwise-2026-09-11")  # gemma-4-31b bench traces
D = sys.argv[1]; BASE = sys.argv[2]
def sb(r): return 2 * r / (1 + r)
def halves(traces, name, mode):
    s = [summarize([t for t in traces if t["presentation"] == p], [name], 40, mode, 3.0)[0][name]["scores"] for p in (0, 1)]
    return spearman(s[0], s[1])
for label in ("lw", "hn_top"):
    gt = [json.loads(l) for l in open(f"{G}/trace-{label}.jsonl")]; gt = [t for t in gt if t["arm"] == "separate"]
    dt = [json.loads(l) for l in open(f"{D}/trace-{label}.jsonl")]; dt = [t for t in dt if t["arm"] == "separate"]
    base = json.load(open(f"{BASE}/summary-{label}.json")); gsep = [a for a in base["arms"] if a["arm"] == "separate"][0]["per_criterion"]
    names = [t["criteria"][0] for t in gt]; names = list(dict.fromkeys(names))
    for nm in names:
        rg, rs = halves(gt, nm, "greedy"), halves(dt, nm, "marginal")
        full = summarize(dt, [nm], 40, "marginal", 3.0)[0][nm]["scores"]
        rho = spearman(full, gsep[nm]["scores"])
        print(f"{label:8s} {nm:12s} gemma split-half {rg:+.3f} (full {sb(rg):.3f}) | student split-half {rs:+.3f} (full {sb(rs):.3f}) | rho_obs {rho:+.3f} | disattenuated {rho/np.sqrt(sb(rg)*sb(rs)):+.3f}")
