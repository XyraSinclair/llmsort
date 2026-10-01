"""Where a judge's pairwise reads stop being one score per item: an exact energy budget.  (replay only; no key)

python3 holonomy_budget.py   -- reads obs-<cohort>-<variant>.jsonl for arxiv, manifund, lw at k = 2, 4, 8, 12.

Every observation is a log-ratio y = L_c(x, y) read in call c (one window of k items, one attribute).
Its energy sum(y^2) splits into five orthogonal shares, each the gap between nested least-squares fits:
  bias     : the call's mean of the symmetric part (L(x,y) + L(y,x)) / 2, a mention-first offset one number corrects
  order    : the rest of the symmetric part, pair by pair (what a swap-and-return loop exposes beyond the offset)
  curl     : the antisymmetric part left over by one potential per window (cycles inside a window)
  gluing   : left over by one potential per attribute that every window shares, beyond the per-window fit
             (each window is integrable on its own, but the windows' scores disagree on shared items)
  gradient : what one score per item explains
The fits are nested (one shared potential is a per-window potential), so the shares sum to 1 exactly;
the script asserts it independently. holonomy = 1 - gradient; bias alone is correctable by one offset per call.
Jev reads in one deterministic prefill pass, so none of it is sampling noise. At k = 2 a call holds one pair,
so bias absorbs that pair's whole order term and curl is zero by construction; read k = 2 'after bias' as a floor.
"""
import json, os
from collections import defaultdict
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
VARIANTS = [("k=2", "elab-k2"), ("k=4", "elab-k4"), ("k=8", "elab"), ("k=12", "elab-k12")]


def budget(obs):
    """obs: list of (call, i, j, y) for one attribute. Returns energies (bias, order, curl, gluing, gradient)."""
    calls = defaultdict(dict)
    for c, i, j, y in obs:
        calls[c][(i, j)] = y
    bias = order = 0.0
    edges = []  # (call, i, j, a) with i < j, a = antisymmetric part
    for c, d in calls.items():
        sym = []
        for (i, j), y in d.items():
            if i < j:
                y_back = d[(j, i)]  # both orders are always read; a missing one is a data error
                sym.append((y + y_back) / 2)
                edges.append((c, i, j, (y - y_back) / 2))
        sym = np.array(sym)
        bias += 2 * len(sym) * sym.mean() ** 2
        order += 2 * float(((sym - sym.mean()) ** 2).sum())
    a = np.array([e[3] for e in edges])
    total_anti = 2 * float(a @ a)

    def residual_energy(n_vertices, vertex_of):
        # LS fit a_e ~ u[v(i)] - u[v(j)]; returns residual energy (counted over both orders)
        B = np.zeros((len(edges), n_vertices))
        for r, (c, i, j, _) in enumerate(edges):
            B[r, vertex_of(c, i)] += 1
            B[r, vertex_of(c, j)] -= 1
        u, *_ = np.linalg.lstsq(B, a, rcond=None)
        fit = (B * u).sum(axis=1)  # elementwise: numpy 2.0 on Accelerate warns spuriously in matmul
        res = a - fit
        assert abs(float(fit @ res)) <= 1e-9 * total_anti, "projection residual not orthogonal"
        return 2 * float(res @ res), 2 * float(fit @ fit)

    items = sorted({e[1] for e in edges} | {e[2] for e in edges})
    item_ix = {v: k for k, v in enumerate(items)}
    call_items = sorted({(c, i) for c, i, _, _ in edges} | {(c, j) for c, _, j, _ in edges})
    ci_ix = {v: k for k, v in enumerate(call_items)}
    res_window, _ = residual_energy(len(call_items), lambda c, i: ci_ix[(c, i)])
    res_global, grad = residual_energy(len(items), lambda c, i: item_ix[i])
    e = np.array([bias, order, res_window, res_global - res_window, grad])
    total = sum(y * y for d in calls.values() for y in d.values())
    assert abs(e.sum() - total) <= 1e-9 * total and (e >= -1e-12 * total).all(), "budget does not close"
    return e


print("share of judgement energy (mean over 12 attributes); holonomy = 1 - gradient;")
print("after bias = holonomy once the mention offset is corrected, as a share of what remains (range over attributes)\n")
print(f"{'cohort':9s} {'instr':6s} {'k':>4s}  {'bias':>6s} {'order':>6s} {'curl':>6s} {'gluing':>7s} {'gradient':>9s}  {'holonomy':>9s}  {'after bias':>10s}")
for name in ("arxiv", "manifund", "lw"):
    for ins in ("score9", "noul"):
        for k_label, var in VARIANTS:
            path = f"{HERE}/obs-{name}-{var}.jsonl"
            if not os.path.exists(path):
                continue
            by_attr = defaultdict(list)
            for line in open(path):
                o = json.loads(line)
                if o["instr"] == ins:
                    by_attr[o["attr"]].append((o["call"], o["i"], o["j"], o["y"]))
            if not by_attr:
                continue
            shares = np.array([(e := budget(v)) / e.sum() for v in by_attr.values()])
            m = shares.mean(axis=0)
            h = 1 - shares[:, 4]
            hb = (shares[:, 1:4].sum(axis=1)) / (1 - shares[:, 0])
            print(f"{name:9s} {ins:6s} {k_label[2:]:>4s}  {m[0]:6.3f} {m[1]:6.3f} {m[2]:6.3f} {m[3]:7.3f} {m[4]:9.3f}  {h.mean():9.3f}  {hb.mean():10.3f} [{hb.min():.2f}–{hb.max():.2f}]")
    print()
