"""Renders report.html from res-*.json and drift-*.json. python3 report.py (no key)."""
import glob, json, math, os
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__)); J = lambda f: json.load(open(f"{HERE}/{f}")); f2 = lambda v: f"{v:.2f}".replace("0.", ".", 1) if abs(v) < 1 else f"{v:.2f}"
f3 = lambda v: f"{v:.3f}".lstrip("0"); usd = lambda v: f"${v:.4f}"
COH = [("countries", "198 countries by population", "vs log population"), ("arxiv150", "150 arXiv abstracts by novelty", "vs gemma-4-31b, 40 items")]
REC = [("rate", "rate k8", "#2f6f73", ""), ("rate-k24", "rate k24", "#2f6f73", "6 3"), ("rate-k4", "rate k4", "#2f6f73", "2 2"), ("arate", "adaptive rate k8", "#7fb3b5", ""), ("arate-k24", "adaptive rate k24", "#7fb3b5", "6 3"),
       ("noul", "yes/no all pairs", "#b4552d", ""), ("score9", "ratio all pairs", "#8a2f2f", ""), ("chain", "ratio cycle", "#6b5ca5", ""), ("achain", "adaptive ratio cycle", "#a99bd6", ""),
       ("anchor", "anchored wide ratio k8", "#3f7d3a", ""), ("anchor-k24", "anchored wide ratio k24", "#3f7d3a", "6 3")]
RES = {c: {r: J(f"res-{c}-{r}.json") for r, *_ in REC if os.path.exists(f"{HERE}/res-{c}-{r}.json")} for c, *_ in COH}
DRIFT = {c: J(f"drift-{c}.json") for c in ("arxiv", "manifund", "lw")}
CAS = J("cascade.json")
TRUTH = [("countries", "countries · population"), ("elements", "elements · atomic number"), ("films", "films · box office"), ("mountains", "mountains · elevation"), ("cities", "cities · population"), ("rivers", "rivers · length"), ("companies", "companies · revenue")]
TR = {c: {r: J(f"res-{c}-{r}.json") for r in ("rate-k24", "noul", "anchor")} for c, _ in TRUTH}
N = {c: RES[c]["rate"]["n"] for c, *_ in COH}
THR = {"countries": ("rho", .97), "arxiv150": ("self", .95)}


def spend():
    t = 0
    fs = glob.glob(f"{HERE}/trace-*.jsonl"); fs = fs or glob.glob(f"{HERE}/trace-*.jsonl.gz")
    for f in fs:
        import gzip
        for l in (gzip.open(f, "rt") if f.endswith(".gz") else open(f)):
            t += json.loads(l)["usage"]["input_tokens"]
    return t * 0.042 / 1e6


def reach(c, r, key=None, thr=None):
    key, thr = THR[c] if key is None else (key, thr)
    for row in RES[c][r]["rounds"]:
        v = row[key]
        if v == v and v >= thr:
            return row["dollars"], row["round"]
    return None, None


def per_round(c, r):
    rows = RES[c][r]["rounds"]; return rows[0]["dollars"] if r != "anchor" and r != "anchor-k24" else rows[1]["dollars"] - rows[0]["dollars"]


def bits(r):
    return 0.5 * math.log2(1 / max(1 - r * r, 1e-9))


def curves_fig(c, key, ylo, yhi, ticks, title):
    W, H, L, B, R = 430, 260, 40, 34, 118; rows = RES[c]
    lo, hi = math.log10(0.001), math.log10(0.07)
    x = lambda d: L + (math.log10(d) - lo) / (hi - lo) * (W - L - R); y = lambda v: H - B - (v - ylo) / (yhi - ylo) * (H - B - 18)
    s = f'<svg viewBox="0 0 {W} {H}" role="img" aria-label="{title}"><text x="{L}" y="12" class="lab" style="font-weight:600">{title}</text>'
    for t in ticks: s += f'<line x1="{L}" y1="{y(t):.1f}" x2="{W - R}" y2="{y(t):.1f}" class="grid"/><text x="{L - 5}" y="{y(t) + 4:.1f}" class="tick" text-anchor="end">{f2(t)}</text>'
    for d in (0.001, 0.003, 0.01, 0.03): s += f'<line x1="{x(d):.1f}" y1="{y(yhi):.1f}" x2="{x(d):.1f}" y2="{H - B}" class="grid"/><text x="{x(d):.1f}" y="{H - B + 13}" class="tick" text-anchor="middle">{d:g}</text>'
    s += f'<text x="{(L + W - R) / 2:.1f}" y="{H - 4}" class="tick" text-anchor="middle">dollars spent (log)</text>'
    ends = []
    for r, lab, col, dash in REC:
        if r not in rows: continue
        pts = [(row["dollars"], row[key]) for row in rows[r]["rounds"] if row[key] == row[key] and row["dollars"] >= 0.001]
        pts = [(d, max(min(v, yhi), ylo)) for d, v in pts]
        if not pts: continue
        s += f'<polyline points="{" ".join(f"{x(d):.1f},{y(v):.1f}" for d, v in pts)}" fill="none" stroke="{col}" stroke-width="1.8"' + (f' stroke-dasharray="{dash}"' if dash else "") + "/>"
        s += "".join(f'<circle cx="{x(d):.1f}" cy="{y(v):.1f}" r="2.2" fill="{col}"/>' for d, v in pts)
        ends.append((y(pts[-1][1]), x(pts[-1][0]), lab, col))
    ends.sort(); last = -99
    for yy, xx, lab, col in ends:  # spread labels
        yy = max(yy, last + 10.5); last = yy
        s += f'<text x="{W - R + 4}" y="{yy + 3.5:.1f}" class="tick" style="fill:{col};font-size:9.5px">{lab}</text>'
    return s + "</svg>"


def reach_fig():
    W, rh = 860, 17; H = 40 + len(REC) * rh; out = ""
    for c, name, refl in COH:
        key, thr = THR[c]; vals = [(lab, col, reach(c, r)[0]) for r, lab, col, _ in REC if r in RES[c]]
        vals = sorted(vals, key=lambda v: (v[2] is None, v[2] or 0))
        hi = max(v for _, _, v in vals if v) * 1.15; x = lambda d: 200 + d / hi * (W - 260)
        s = f'<svg viewBox="0 0 {W} {H}" role="img" aria-label="dollars to threshold, {name}"><text x="0" y="12" class="lab" style="font-weight:600">{name}: dollars to {"ρ" if key == "rho" else "self-agreement"} ≥ {f2(thr)} {refl if key == "rho" else ""}</text>'
        for i, (lab, col, v) in enumerate(vals):
            yy = 30 + i * rh
            s += f'<text x="195" y="{yy + 4}" class="tick" text-anchor="end">{lab}</text>'
            if v: s += f'<rect x="200" y="{yy - 6}" width="{x(v) - 200:.1f}" height="12" fill="{col}"/><text x="{x(v) + 5:.1f}" y="{yy + 4}" class="tick">{usd(v)}</text>'
            else: s += f'<text x="204" y="{yy + 4}" class="tick">not reached in the budget</text>'
        out += s + "</svg>"
    return out


def slope_fig():
    W, rh = 860, 17; rows = RES["countries"]; vals = sorted([(lab, col, rows[r]["rounds"][-1]["slope"]) for r, lab, col, _ in REC if r in rows and not r.startswith(("rate", "arate"))], key=lambda v: -v[2])
    H = 40 + len(vals) * rh; x = lambda v: 200 + v / 1.0 * (W - 300)
    s = f'<svg viewBox="0 0 {W} {H}" role="img" aria-label="cardinal slope"><text x="0" y="12" class="lab" style="font-weight:600">countries: fitted log-ratio per nat of true log population (1.0 = calibrated)</text>'
    s += f'<line x1="{x(1):.1f}" y1="20" x2="{x(1):.1f}" y2="{H - 8}" class="grid" stroke-dasharray="3 3"/><text x="{x(1):.1f}" y="{H - 2}" class="tick" text-anchor="middle">1.0</text>'
    for i, (lab, col, v) in enumerate(vals):
        yy = 30 + i * rh
        s += f'<text x="195" y="{yy + 4}" class="tick" text-anchor="end">{lab}</text><rect x="200" y="{yy - 6}" width="{x(v) - 200:.1f}" height="12" fill="{col}"/><text x="{x(v) + 5:.1f}" y="{yy + 4}" class="tick">{f2(v)}</text>'
    return s + "</svg>"


def table():
    h = '<table><thead><tr><th>cohort</th><th>recipe</th><th>k</th><th>questions / call</th><th>$ / round</th><th>rounds</th><th>$ total</th><th>final ρ</th><th>final self</th><th>bits / item</th><th>$ to threshold</th><th>slope</th></tr></thead><tbody>'
    for c, name, refl in COH:
        first = True
        for r, lab, col, _ in REC:
            if r not in RES[c]: continue
            d = RES[c][r]; last = d["rounds"][-1]; k = d["k"]; q = {"rate": k, "arate": k, "noul": k * (k - 1), "score9": k * (k - 1), "chain": 2 * k, "achain": 2 * k, "anchor": 3 * (k - 3) + 3}[r.split("-")[0]]
            dt, rt = reach(c, r); b = bits(last["pearson"]) if c == "countries" else bits(math.sqrt(max(last["self"], 0)))
            h += (f'<tr><td>{name if first else ""}</td><td style="color:{col}">{lab}</td><td>{k}</td><td>{q}</td><td>{usd(per_round(c, r))}</td><td>{last["round"]}</td><td>{usd(last["dollars"])}</td><td>{f3(last["rho"])}</td><td>{f3(last["self"])}</td>'
                  f'<td>{b:.1f}</td><td>{usd(dt) + f" (round {rt})" if dt else "—"}</td><td>{f2(last["slope"]) if last["slope"] is not None else "—"}</td></tr>'); first = False
    return h + "</tbody></table>"


def drift_table():
    h = '<table><thead><tr><th>cohort</th><th>calls re-asked</th><th>$</th><th>yes/no p: r day vs day</th><th>mean |Δp|</th><th>moved &gt; .05</th><th>ratio E: r</th><th>mean |ΔE| (levels)</th><th>latents day vs day</th><th>vs Fable, day 1 → 3</th></tr></thead><tbody>'
    for c, n in (("arxiv", "arXiv abstracts"), ("manifund", "Manifund applications"), ("lw", "LessWrong comments")):
        d = DRIFT[c]; l = d["latent"]
        h += (f'<tr><td>{n}</td><td>{d["calls"]}</td><td>{d["tokens"] * 0.042 / 1e6:.3f}</td><td>{f3(d["noul"]["pearson"])}</td><td>{d["noul"]["mad"]:.3f}</td><td>{d["noul"]["moved05"] * 100:.1f} %</td><td>{f3(d["score"]["pearson"])}</td><td>{d["score"]["mad"]:.2f}</td>'
              f'<td>{" / ".join(f3(l[i]["day_vs_day"]) for i in ("noul", "score9", "rate"))}</td><td>{" / ".join(f2(l[i]["fable_day1"]) + "→" + f2(l[i]["fable_day3"]) for i in ("noul", "score9", "rate"))}</td></tr>')
    return h + "</tbody></table>"


def ceiling_fig():
    W, rh = 860, 22; H = 44 + len(TRUTH) * rh; x = lambda v: 230 + (v - .5) / .5 * (W - 300)
    s = f'<svg viewBox="0 0 {W} {H}" role="img" aria-label="knowledge ceiling by cohort"><text x="0" y="12" class="lab" style="font-weight:600">ρ against the fact, final round (filled) and self-agreement (hollow), seven cohorts</text>'
    for t in (.5, .6, .7, .8, .9, 1.0): s += f'<line x1="{x(t):.1f}" y1="20" x2="{x(t):.1f}" y2="{H - 14}" class="grid"/><text x="{x(t):.1f}" y="{H - 2}" class="tick" text-anchor="middle">{f2(t) if t < 1 else "1.0"}</text>'
    for i, (c, lab) in enumerate(TRUTH):
        yy = 34 + i * rh; s += f'<text x="222" y="{yy + 4}" class="tick" text-anchor="end">{lab}</text>'
        for r, col, dy in (("rate-k24", "#2f6f73", -4), ("noul", "#b4552d", 0), ("anchor", "#3f7d3a", 4)):
            l = TR[c][r]["rounds"][-1]
            s += f'<circle cx="{x(l["rho"]):.1f}" cy="{yy + dy}" r="4" fill="{col}"/><circle cx="{x(l["self"]):.1f}" cy="{yy + dy}" r="3.5" fill="var(--bg)" stroke="{col}" stroke-width="1.5"/>'
    return s + "</svg>"


WD = {c: J(f"wordings-{c}.json") for c in ("countries", "names")}; REFN = J("ref-names.json")
WROWS = [("countries", "population", "population (fact)"), ("names", "vc", "VC respectability"), ("names", "rounds", "rounds a founder raises"), ("names", "aura", "aura"), ("names", "gay", "sounds gay")]
WORDS = ["plain", "persona", "numeric", "casual", "negated", "formal", "spin"]
wd = lambda c, a: WD[c][a]


def wording_fig():
    W, rh = 860, 24; H = 70 + len(WROWS) * rh; x = lambda v: 200 + (v + 1) / 2 * (W - 260)
    s = f'<svg viewBox="0 0 {W} {H}" role="img" aria-label="wording invariance"><text x="0" y="12" class="lab" style="font-weight:600">ρ of each wording (own windows) with the canonical wording on other windows; tick = canonical vs itself on other windows</text>'
    for t in (-1, -.5, 0, .5, 1): s += f'<line x1="{x(t):.1f}" y1="22" x2="{x(t):.1f}" y2="{H - 36}" class="grid"/><text x="{x(t):.1f}" y="{H - 24}" class="tick" text-anchor="middle">{t:g}</text>'
    cols = {"plain": "#2f6f73", "persona": "#b4552d", "numeric": "#6b5ca5", "casual": "#7fb3b5", "negated": "#8a2f2f", "formal": "#3f7d3a", "spin": "#c9a227"}
    for i, (c, a, lab) in enumerate(WROWS):
        yy = 36 + i * rh; o = wd(c, a); s += f'<text x="192" y="{yy + 4}" class="tick" text-anchor="end">{lab}</text>'
        m = float(np.mean(o["same_wording"])); s += f'<line x1="{x(m):.1f}" y1="{yy - 9}" x2="{x(m):.1f}" y2="{yy + 9}" stroke="var(--ink)" stroke-width="2"/>'
        for w in WORDS: s += f'<circle cx="{x(o["fresh_window"][w]):.1f}" cy="{yy}" r="4.5" fill="{cols[w]}" fill-opacity=".85"><title>{w} {f2(o["fresh_window"][w])}</title></circle>'
    yy = H - 6; xx = 200
    for w, col in cols.items(): s += f'<circle cx="{xx}" cy="{yy}" r="4" fill="{col}"/><text x="{xx + 7}" y="{yy + 4}" class="tick">{w}</text>'; xx += 78
    return s + "</svg>"


def wording_table():
    h = '<table><thead><tr><th>attribute</th><th>reference</th><th>same win.</th><th>fresh win.</th><th>one wording</th><th>self 8w</th><th>self 7w+</th><th>self 8r</th><th>ρ 8w</th><th>ρ 7w+</th><th>ρ 8r</th></tr></thead><tbody>'
    for c, a, lab in WROWS:
        o = wd(c, a); pos = [w for w in WORDS if w != "negated"]
        rs = "log population" if c == "countries" else "Fable 5.1 × 2"
        h += f'<tr><td>{lab}</td><td>{rs}</td><td>{f2(np.mean([o["same_window"][w] for w in pos]))}</td><td>{f2(np.mean([o["fresh_window"][w] for w in pos]))}</td><td>{f2(np.mean(o["same_wording"]))}</td><td>{f3(o["self_diverse"])}</td><td>{f3(o["self_diverse_pos"])}</td><td>{f3(o["self_repeated"])}</td><td>{f2(o["pooled_diverse_vs_ref"])}</td><td>{f2(o["pooled_diverse_pos_vs_ref"])}</td><td>{f2(o["pooled_repeated_vs_ref"])}</td></tr>'
    return h + "</tbody></table>"


def names_table():
    h = '<table><thead><tr><th>attribute</th><th>Fable A vs B</th>' + "".join(f"<th>{w}</th>" for w in ["canonical"] + WORDS) + '</tr></thead><tbody>'
    for c, a, lab in WROWS[1:]:
        o = wd(c, a); h += f'<tr><td>{lab}</td><td>{f2(REFN["replica_agreement"][a])}</td>' + "".join(f"<td>{f2(o['vs_ref'][w])}</td>" for w in ["canonical"] + WORDS) + "</tr>"
    return h + "</tbody></table>"


import grid as G
GCOH = [("countries", "countries · population", "log population"), ("films", "films · box office", "log gross"), ("companies", "companies · revenue", "log revenue"), ("names-aura", "names · aura", "Fable 5.1 × 2")]
GRID = {c: {r: J(f"grid-{c}-{r}.json") for r in G.RECIPES if os.path.exists(f"{HERE}/grid-{c}-{r}.json")} for c, *_ in GCOH}
FAM = {"rate": ("setwise rating", "#2f6f73"), "tophalf": ("per-item yes/no", "#7fb3b5"), "top": ("choice: highest", "#c9a227"), "topbot": ("choice: highest + lowest", "#c9a227"), "tri": ("triples in a window", "#6b5ca5"), "tri3": ("three-item states", "#6b5ca5"),
       "noul": ("yes/no all pairs", "#b4552d"), "noulcyc": ("yes/no cycle", "#b4552d"), "score9": ("ratio all pairs", "#8a2f2f"), "chain": ("ratio cycle", "#8a2f2f"), "score5cyc": ("five-rung ratio cycle", "#8a2f2f"), "widecyc": ("wide ratio cycle", "#8a2f2f"),
       "pair2": ("two-item states, yes/no", "#b4552d"), "pair2r": ("two-item states, ratio", "#8a2f2f"), "anchor": ("anchored wide ratio", "#3f7d3a")}
WIN = "rate-L10-k24"; BUDGETS = (0.002, 0.005, 0.01)


def rho_at(rows, d):
    xs = [r["dollars"] for r in rows]; ys = [r["rho"] for r in rows]
    if d < xs[0]: return float("nan")
    return ys[-1] if d >= xs[-1] else float(np.interp(math.log(d), np.log(xs), ys))


def grid_score(r):
    v = float(np.nanmean([rho_at(GRID[c][r]["rounds"], .005) for c in GRID if r in GRID[c]])); return v if v == v else -1


def gval(c, r, d):
    return GRID[c][r]["rounds"][-1]["rho"] if d == "final" else rho_at(GRID[c][r]["rounds"], d)


def deficit(r):
    """Largest amount by which any recipe beats r at any budget or the final round, on any cohort: (gap, recipe, cohort, budget)."""
    worst = (-1, None, None, None)
    for c, *_ in GCOH:
        for d in list(BUDGETS) + ["final"]:
            w = gval(c, r, d)
            if w != w: continue
            for o in GRID[c]:
                v = gval(c, o, d)
                if v == v and v - w > worst[0]: worst = (v - w, o, c, d)
    return worst


def grid_fig():
    W, H, L, B, R = 430, 250, 40, 34, 14; lo, hi = math.log10(0.0004), math.log10(0.06); out = ""
    for c, name, refl in GCOH:
        rows = GRID[c]; ylo = {"countries": .8, "films": .7, "companies": .5, "names-aura": .4}[c]; yhi = 1.0
        x = lambda d: L + (math.log10(d) - lo) / (hi - lo) * (W - L - R); y = lambda v: H - B - (max(v, ylo) - ylo) / (yhi - ylo) * (H - B - 18)
        s = f'<svg viewBox="0 0 {W} {H}" role="img" aria-label="{name}"><text x="{L}" y="12" class="lab" style="font-weight:600">{name}: ρ vs {refl}</text>'
        for t in np.arange(ylo, 1.001, .1): s += f'<line x1="{L}" y1="{y(t):.1f}" x2="{W - R}" y2="{y(t):.1f}" class="grid"/><text x="{L - 5}" y="{y(t) + 4:.1f}" class="tick" text-anchor="end">{f2(t) if t < 1 else "1.0"}</text>'
        for d in (0.001, 0.003, 0.01, 0.03): s += f'<line x1="{x(d):.1f}" y1="{y(yhi):.1f}" x2="{x(d):.1f}" y2="{H - B}" class="grid"/><text x="{x(d):.1f}" y="{H - B + 13}" class="tick" text-anchor="middle">{d:g}</text>'
        s += f'<text x="{(L + W - R) / 2:.1f}" y="{H - 4}" class="tick" text-anchor="middle">dollars spent (log)</text>'
        for r in sorted(rows, key=lambda r: r == WIN):
            pts = [(row["dollars"], row["rho"]) for row in rows[r]["rounds"]]; col = FAM[G.RECIPES[r][1]][1]; win = r == WIN
            s += f'<polyline points="{" ".join(f"{x(d):.1f},{y(v):.1f}" for d, v in pts)}" fill="none" stroke="{"var(--ink)" if win else col}" stroke-width="{3 if win else 1.1}" stroke-opacity="{1 if win else .55}"><title>{r}</title></polyline>'
        out += s + "</svg>"
    return out


def grid_table():
    h = '<table><thead><tr><th>recipe</th><th>family</th><th>k</th><th>q / call</th><th>$ / round</th>' + "".join(f'<th>{c.split("-")[0]}</th>' for c, *_ in GCOH) + '<th>mean</th><th>worst gap</th><th>self</th><th>slope</th></tr></thead><tbody>'
    for r in sorted(G.RECIPES, key=lambda r: (-grid_score(r), deficit(r)[0])):
        if not all(r in GRID[c] for c, *_ in GCOH): continue
        fam, col = FAM[G.RECIPES[r][1]]; d = GRID["countries"][r]; sty = ' style="font-weight:700"' if r == WIN else ""; sc = grid_score(r)
        h += f'<tr{sty}><td>{r}</td><td style="color:{col}">{fam}</td><td>{d["k"]}</td><td>{d["q_per_call"]}</td><td>{usd(d["rounds"][0]["dollars"])}</td>'
        for c, *_ in GCOH:
            v = rho_at(GRID[c][r]["rounds"], .005); h += f'<td>{f2(v) if v == v else "—"} / {f3(GRID[c][r]["rounds"][-1]["rho"])}</td>'
        h += f'<td>{f3(sc) if sc >= 0 else "—"}</td><td>{f3(deficit(r)[0])}</td><td>{f2(np.mean([GRID[c][r]["rounds"][-1]["self"] for c, *_ in GCOH]))}</td><td>{f2(d["rounds"][-1]["slope"])}</td></tr>'
    return h + "</tbody></table>"


def cascade_table():
    qs = ["0", "2", "4", "6", "8", "12", "16", "24"]; h = '<table><thead><tr><th>cohort</th><th>signal</th>' + "".join(f"<th>q = {q}</th>" for q in qs) + '</tr></thead><tbody>'
    for c, n in (("arxiv", "arXiv abstracts"), ("manifund", "Manifund applications"), ("lw", "LessWrong comments")):
        d = CAS[c]; first = True
        for k, lab in (("cascade", "Jev round-spread"), ("gem_cascade", "gemma disagreement"), ("random", "random"), ("oracle", "oracle")):
            h += f'<tr><td>{n if first else ""}</td><td>{lab}</td>' + "".join(f"<td>{f2(d[k][q])}</td>" for q in qs) + "</tr>"; first = False
    return h + "</tbody></table>"


CSS = """
:root{--bg:#faf7f1;--ink:#1c1a17;--mute:#6d675d;--rule:#d9d2c4;--card:#f2ede3;--acc:#2f6f73}
@media(prefers-color-scheme:dark){:root{--bg:#15161a;--ink:#e8e4da;--mute:#9a9487;--rule:#33353c;--card:#1d1f25;--acc:#6fb7bb}}
*{box-sizing:border-box}html{background:var(--bg);color:var(--ink);font:16px/1.55 Charter,'Iowan Old Style','Palatino Linotype',Georgia,serif;-webkit-text-size-adjust:100%}
body{margin:0 auto;padding:56px 28px 96px;max-width:920px}
h1{font-size:34px;line-height:1.15;margin:0 0 10px;letter-spacing:-.01em;font-weight:600}
h2{font-size:13px;letter-spacing:.14em;text-transform:uppercase;font-family:ui-sans-serif,-apple-system,'Helvetica Neue',sans-serif;color:var(--mute);font-weight:600;margin:52px 0 14px;padding-top:14px;border-top:1px solid var(--rule)}
p{margin:0 0 12px}.sub{color:var(--mute);font-size:15px;margin-bottom:26px}
.verdict{background:var(--card);border-left:3px solid var(--acc);padding:16px 20px;margin:22px 0}.verdict p:last-child{margin:0}
.so{color:var(--mute);font-style:italic}.so b{font-style:normal;color:var(--ink);font-weight:600}
code{font:13px/1.4 ui-monospace,'SF Mono',Menlo,monospace;background:var(--card);padding:1px 5px;border-radius:3px}
table{border-collapse:collapse;font:12.5px/1.3 ui-sans-serif,-apple-system,'Helvetica Neue',sans-serif;font-variant-numeric:tabular-nums;width:100%;margin:12px 0 6px}
th,td{padding:5px 6px;text-align:right;white-space:nowrap;border-bottom:1px solid var(--rule)}th:first-child,td:first-child,th:nth-child(2),td:nth-child(2){text-align:left}thead th{color:var(--mute);font-weight:600;font-size:11px;letter-spacing:.03em}
figure{margin:18px 0 22px}figcaption{font:13px/1.45 ui-sans-serif,-apple-system,sans-serif;color:var(--mute);margin-top:6px}
.two{display:grid;grid-template-columns:1fr 1fr;gap:14px}@media(max-width:760px){.two{grid-template-columns:1fr}}
svg{width:100%;height:auto;display:block;font-family:ui-sans-serif,-apple-system,'Helvetica Neue',sans-serif}
svg .grid{stroke:var(--rule);stroke-width:1}svg .tick{fill:var(--mute);font-size:11.5px}svg .lab{fill:var(--ink);font-size:13.5px}
ol{padding-left:22px}li{margin-bottom:8px}
.foot{color:var(--mute);font-size:14px}
"""

TOTAL = spend()
_jg = [v for c in CAS.values() for v in c["attr_jg"]]; _jB = [v for c in CAS.values() for v in c["attr_jB"]]; CAS_ATTR_R = float(np.corrcoef(_jg, _jB)[0, 1])
rc = {r: reach("countries", r) for r in RES["countries"]}; ra = {r: reach("arxiv150", r) for r in RES["arxiv150"]}
fin = lambda c, r, k: RES[c][r]["rounds"][-1][k]
cheap_c = min(rc, key=lambda r: rc[r][0] or 9); cheap_a = min(ra, key=lambda r: ra[r][0] or 9)
ratio_x = rc["score9"][0] / rc["rate"][0]
c_rho = [fin("countries", r, "rho") for r in RES["countries"]]; c_self = [fin("countries", r, "self") for r in RES["countries"] if r != "arate-k24"]
a_rho = [fin("arxiv150", r, "rho") for r in RES["arxiv150"]]; a_self = [fin("arxiv150", r, "self") for r in RES["arxiv150"] if r != "arate-k24"]
slopes = {r: fin("countries", r, "slope") for r in RES["countries"]}
r1 = lambda c, r: RES[c][r]["rounds"][0]
bpd = lambda c, r: N[c] * bits(r1(c, r)["pearson"]) / r1(c, r)["dollars"]  # bits about the reference per dollar, first round, countries
drift_r = [DRIFT[c]["noul"]["pearson"] for c in DRIFT] + [DRIFT[c]["score"]["pearson"] for c in DRIFT]; drift_l = [DRIFT[c]["latent"][i]["day_vs_day"] for c in DRIFT for i in ("noul", "score9", "rate")]

HTML = f"""<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Sorting with Jev: which recipe buys the most information per dollar?</title><style>{CSS}</style>
<h1>Sorting with Jev: which recipe buys the most information per dollar?</h1>
<p class="sub">Eleven sorting recipes on hosted Jev (TypeSafe <code>jev-latest</code>, $0.042 per million input tokens), run round by round on 198 countries by population (a true cardinal reference) and 150 arXiv abstracts by novelty (a generating judge as reference), each checkpoint priced; the same 324 windows re-asked two days apart; one attribute asked eight ways on countries and on 169 first names by four perceived qualities against a frontier judge; and a closing grid of {len(G.RECIPES)} judgment designs — pairwise, triple, setwise, choice, yes/no, ladders, level counts, state sizes, formats — on four cohorts at equal spend. Measured 2026-09-21 and 22. ${TOTAL:.2f} in total. Written for anyone who ranks things with model judgments and pays per token.</p>

<div class="verdict">
<p><b>The best sorting system on Jev, across every way of asking — one item rated among many, two at a time, three at a time, choose-the-highest, yes/no, ratio ladders, 3 to 10 levels, states of 2 to 48 items — is a 24-item labelled state with one ten-level standing question per item, random windows, repeated.</b> On the § 10 grid ({len(G.RECIPES)} recipes × four cohorts × every budget) no other recipe beats it by more than {f3(deficit(WIN)[0])} anywhere — inside the judge’s own .03 wobble — and every other family loses to it by .05–.35 somewhere, most of all on the soft attribute, where pairwise, anchored and choice designs sit .1–.35 below it. It costs {usd(GRID["countries"][WIN]["rounds"][0]["dollars"])} a round for 200 items and reaches its ceiling in two.</p>
<p><b>How good is Jev at pairwise ratio sorting: as good as the reference it is checked against, transitive, cheap, and cardinal only when anchored.</b> Every recipe reaches the same ceiling — ρ {f2(min(c_rho))}–{f2(max(c_rho))} against true population, {f2(min(a_rho))}–{f2(max(a_rho))} against gemma-4-31b on abstracts (gemma’s own reliability), self-agreement {f2(min(c_self))}–{f2(max(c_self))} and {f2(min(a_self))}–{f2(max(a_self))} — so the ratio ladder is not more accurate than a yes/no or a rating; it is one of several ways to read the same latent. Its fitted log-ratios are compressive: {f2(slopes["score9"])} nat per nat of true log population on a 1/8…8 ladder, {f2(slopes["anchor"])} on a 1/100…100 ladder against three pinned anchors.</p>
<p><b>The recipe that buys the most information per dollar is the plainest one: k items in a window, one ten-level rating each, random windows, repeat.</b> ρ ≥ .97 on countries costs {usd(rc["rate-k24"][0])} at k = 24 and {usd(rc["rate"][0])} at k = 8; self-agreement ≥ .95 on 150 abstracts costs {usd(ra[cheap_a][0])}. The all-pairs ratio ladder reaches the same numbers at {ratio_x:.0f}× the price; yes/no on all pairs at {rc["noul"][0] / rc["rate"][0]:.1f}×; a ratio cycle (2k questions) at {rc["chain"][0] / rc["rate"][0]:.0f}×. Adaptive windows (sort, then compare neighbours) bought nothing here because the ceiling is reached in two or three random rounds.</p>
<p><b>Pinned anchors with a wide ladder are the recipe for a cardinal answer,</b> and the most self-consistent one (self {f3(fin("arxiv150", "anchor", "self"))} on abstracts, {f3(fin("countries", "anchor", "self"))} on countries); a round costs {per_round("countries", "anchor") / per_round("countries", "rate"):.1f}× a rating round, but it clears the accuracy threshold in its first, so the cost to threshold ({usd(rc["anchor"][0])}) is within 1.5× of the rating’s. <b>Time drift is nil:</b> the same request two days later returns the same probabilities (r {f3(min(drift_r))}–{f3(max(drift_r))} over 39,000 answers), and the sorts fitted from each day agree at {f3(min(drift_l))}+, so a cached answer is a permanent measurement. <b>Wording is the perturbation that matters,</b> and on a fact it does not: rewordings of “population” agree with each other as well as one wording agrees with itself across window draws. On soft attributes (which names read as VC-respectable, as having aura) rewordings agree at .5–.9, negation is ignored outright, a persona replaces the question, and no pooling of wordings closes the .6–.75 ceiling against Fable — the definition-carrying wording, repeated, is the recipe.</p>
</div>

<h2>Setup</h2>
<p><b>Cohorts.</b> <i>countries</i>: 198 sovereign states from Wikidata with their latest population; the reference is log population, so Spearman measures accuracy against a fact and the fitted slope measures cardinal calibration. <i>arxiv150</i>: 150 arXiv abstracts scored on a one-paragraph definition of novelty; the reference is gemma-4-31b’s pooled z-score on the 40 that overlap the multi-criteria benchmark, so ρ is judge–judge and self-agreement is the precision statistic for all 150.</p>
<p><b>Recipes.</b> A round shows every item once, in windows of k. <i>rate</i>: one ten-level standing question per item (“where does item x stand among those shown”), k questions a call. <i>yes/no all pairs</i>: “x is stronger than y” for every ordered pair, k(k−1). <i>ratio all pairs</i>: the polarity-safe nine-rung ladder (“far weaker: 1/8 or less” … “far stronger: 8 times or more”) for every ordered pair. <i>ratio cycle</i>: the same ladder on a random Hamiltonian cycle only, both orders, 2k. <i>adaptive</i> variants re-fit after each round and window consecutive blocks of the current order with a random offset. <i>anchored wide ratio</i>: three items pinned at the 10 / 50 / 90 % points of a one-round rating pilot (charged) sit in every window beside k−3 targets, each target against each anchor and the anchors against each other on a 1/100 … 100 ladder.</p>
<p><b>Fit and score.</b> Pairs are least squares on the directed log-ratio (ladder expectation) or logit (yes/no); ratings are least squares with a window fixed effect, which is what makes sorted windows usable. After every round: Spearman and Pearson against the reference, split-half by round parity stepped up by Spearman–Brown (<i>self</i>), bits per item as ½ log₂ 1/(1−r²), and the dollars billed so far. The first round of adaptive recipes is random by construction.</p>

<h2>1 · Agreement against dollars</h2>
<div class="two">{curves_fig("countries", "rho", .85, 1.0, (.85, .9, .95, 1.0), "countries: ρ vs true log population")}{curves_fig("arxiv150", "rho", .7, .95, (.7, .75, .8, .85, .9, .95), "arxiv150: ρ vs gemma-4-31b (40 items)")}</div>
<div class="two">{curves_fig("countries", "self", .85, 1.0, (.85, .9, .95, 1.0), "countries: self-agreement")}{curves_fig("arxiv150", "self", .85, 1.0, (.85, .9, .95, 1.0), "arxiv150: self-agreement")}</div>
<figure><figcaption>One point per round; the x axis is cumulative dollars, log scale. Self-agreement is undefined at round 1 and depressed at rounds 2–3, where one half is a single round of disjoint windows (the fit connects them only through the window effect); the anchored recipes are connected from round 1, which is part of their self-agreement lead. Teal: rating family; orange and red: all-pairs yes/no and ratio; purple: ratio cycles; green: anchored. Dashed: k = 24, dotted: k = 4.</figcaption></figure>
<p>On countries every recipe climbs to the same ceiling and the x position of the climb is the whole result: rating at k = 24 is at ρ .97 after {rc["rate-k24"][1]} rounds and {usd(rc["rate-k24"][0])}, rating at k = 8 after {rc["rate"][1]} rounds, the all-pairs ratio after {rc["score9"][1]} rounds but {usd(rc["score9"][0])}, because a round of 56 ladder questions on 8 items costs {per_round("countries", "score9") / per_round("countries", "rate"):.0f}× a round of 8 ratings and carries no more about the order. Yes/no on all pairs is the cheap all-pairs form (a yes/no question is a third of a ladder question in tokens) and is the second-cheapest recipe overall. On abstracts the same ordering holds against a softer ceiling: everything sits at ρ .88–.91 against gemma from round 3 on, and the self-agreement panel is where the recipes separate — rating reaches .95 first, the anchored ladder reaches the highest final value.</p>
<p class="so"><b>So:</b> the information per dollar is set by the number of questions a call needs to place its items, not by their form. A rating places k items with k questions; every pairwise design spends at least 2k and gets the same latent.</p>

<h2>2 · Dollars to a threshold</h2>
<figure>{reach_fig()}<figcaption>Cumulative dollars at the first round whose statistic clears the threshold. Countries: accuracy against truth; abstracts: split-half self-agreement of the full 150-item sort (the 40-item ρ is too noisy to threshold). Anchored recipes include the pilot round that picks the anchors.</figcaption></figure>
<p>Read as bits: at its first round the k = 24 rating carries {bpd("countries", "rate-k24") / 1e3:.0f}k bits of population information per dollar ({N["countries"]} items × {bits(r1("countries", "rate-k24")["pearson"]):.1f} bits per item for {usd(r1("countries", "rate-k24")["dollars"])}), k = 8 rating {bpd("countries", "rate") / 1e3:.0f}k, yes/no all pairs {bpd("countries", "noul") / 1e3:.0f}k, the ratio cycle {bpd("countries", "chain") / 1e3:.0f}k, the all-pairs ratio {bpd("countries", "score9") / 1e3:.0f}k. The metric saturates by design — bits per item is bounded by what the judge knows, so past the ceiling every added dollar buys zero — which is why the threshold cost is the operational number.</p>

<h2>3 · Cardinal calibration</h2>
<figure>{slope_fig()}<figcaption>Slope of the fitted latent on true log population, countries, final round; rating recipes are on a level scale and are omitted. A calibrated ratio judge would read 1.0.</figcaption></figure>
<p>The 1/8 … 8 ladder compresses to {f2(slopes["score9"])}–{f2(slopes["chain"])} nat per nat whether asked on all pairs or a cycle, adaptive or not: a country ten times as populous is called “about 2 times” on average. The wide ladder against pinned anchors recovers {f2(slopes["anchor"])} at k = 8 and {f2(slopes["anchor-k24"])} at k = 24, and it is the only recipe whose ratio answers can be read as magnitudes without a calibration step. Yes/no logits are on their own scale ({f2(slopes["noul"])} logit per nat) and are ordinal in practice.</p>
<p class="so"><b>So:</b> for an ordinal answer any recipe is calibrated enough; for a cardinal one, pin anchors, widen the ladder to the range the truth spans, and still expect a quarter of the dynamic range to be missing.</p>

<h2>4 · Adaptive windows</h2>
<p>Sorting after each round and comparing neighbours is the classic way to spend comparisons where the order is uncertain. Here it bought little: adaptive rating reaches the threshold at the same round and price as random windows on both cohorts, and adaptive ratio cycles clear the countries threshold one round earlier ({usd(rc["achain"][0])} against {usd(rc["chain"][0])}), still {rc["achain"][0] / rc["rate"][0]:.0f}× the rating. Two reasons are visible in the data. The ceiling arrives in two or three random rounds, before an adaptive design has anything to exploit. And at k = 24 the adaptive rating’s self-agreement falls to {f3(fin("countries", "arate-k24", "self"))} on countries and {f3(fin("arxiv150", "arate-k24", "self"))} on abstracts against {f3(fin("countries", "rate-k24", "self"))} and {f3(fin("arxiv150", "rate-k24", "self"))} for random windows: windows of near-equal items give a rating question nothing to spread its levels over, and the window fixed effect then absorbs most of what it does say.</p>
<p class="so"><b>So:</b> random windows, repeated, are the design; adaptive refinement is worth revisiting only for a cohort where the plateau takes more than five rounds.</p>

<h2>5 · Time drift</h2>
<figure>{drift_table()}<figcaption>The 108 k = 8 windows per cohort from the 2026-09-19 consistency study (twelve attributes, 24 items, three rounds; 6,048 yes/no and 6,912 ratio answers each) re-issued 2026-09-21 with a cache-busting salt. Latents fitted from each day’s answers; Fable 5.1 is the reference from that study.</figcaption></figure>
<p>Two days apart, a yes/no probability moves on average {np.mean([DRIFT[c]["noul"]["mad"] for c in DRIFT]):.3f} and a nine-level expectation {np.mean([DRIFT[c]["score"]["mad"] for c in DRIFT]):.2f} of a level; the fitted sorts agree at {f3(min(drift_l))} or better and agree with Fable to the same third decimal. That is bf16 batching wobble (the .03 already known), not drift. A cached answer can be reused indefinitely; there is no reason to re-ask a question for freshness.</p>

<h2>6 · Can Jev tell when to hand off?</h2>
<figure>{cascade_table()}<figcaption>jev-highdim data (three cohorts × twelve attributes, 24 items, k = 8, three rounds), no new spend. Escalation: the q items Jev is least sure of are re-placed by an independent Fable read (replica A) on Jev’s scale; the merged order is scored against Fable replica B. <i>oracle</i> escalates the q items that are actually most wrong. Jev’s own signals: the spread of an item’s latent across single-round fits, and low yes/no confidence; <i>gemma</i> escalates where a gemma-4-31b setwise sort disagrees with Jev most.</figcaption></figure>
<p>A system that uses Jev where it can and a stronger model where it cannot needs a signal for “cannot”. Jev does not carry one per item: the rank correlation between its own uncertainty and its error against Fable is {f2(min(CAS[c]["pred_sd"] for c in CAS))}–{f2(max(CAS[c]["pred_conf"] for c in CAS))}, and escalating by it is the random curve. The information exists — an oracle handing over six of 24 items closes most of the gap to Fable — but Jev’s errors are systematic opinions it holds with the same confidence as its correct ones, which is the same fact as “more consistent than right” seen from the other side. Disagreement with a second cheap judge (gemma) is a slightly better signal ({f2(min(CAS[c]["pred_gem"] for c in CAS))}–{f2(max(CAS[c]["pred_gem"] for c in CAS))}) and beats random by a few hundredths on the prose cohorts. At the level of a whole attribute the picture is better: Jev–gemma agreement predicts Jev–Fable agreement at r {CAS_ATTR_R:.2f} over the 36 cells.</p>
<p class="so"><b>So:</b> decide Jev-or-frontier per attribute, from a small pilot against the strong judge, not per item from Jev’s confidence; a second cheap judge is worth more as a disagreement detector than as a second vote.</p>

<h2>7 · Where the ceiling is: seven facts</h2>
<figure>{ceiling_fig()}<figcaption><span style="color:#2f6f73">●</span> rating, k = 24, six rounds · <span style="color:#b4552d">●</span> yes/no all pairs, k = 8, three rounds · <span style="color:#3f7d3a">●</span> anchored wide ratio, k = 8, three rounds. ~200 well-known Wikidata entities per cohort (118 elements), label only, reference log value (atomic number raw). Total for the six new cohorts {usd(sum(TR[c][r]["rounds"][-1]["dollars"] for c, _ in TRUTH if c != "countries" for r in TR[c]))}.</figcaption></figure>
<p>Self-agreement is {f2(min(TR[c]["rate-k24"]["rounds"][-1]["self"] for c, _ in TRUTH))}+ on every cohort; accuracy runs from {f2(TR["companies"]["rate-k24"]["rounds"][-1]["rho"])} (company revenue) to {f2(TR["countries"]["rate-k24"]["rounds"][-1]["rho"])} (country population), and the three recipes agree on the ordering of the cohorts. That is the knowledge-ceiling map the attribute-level switch of § 6 needs: the gap between what Jev repeats and what is true is a property of the fact, not of the recipe, and it is visible in one cheap round. Where the ceiling is low the anchored recipe earns its price early — on cities, mountains and films its first round ({usd(TR["cities"]["anchor"]["rounds"][0]["dollars"])}) is where the rating gets after six — because the pinned anchors connect every window to an absolute reference from the start. The wide ladder’s slope is near 1 where the truth spans about the ladder’s range (mountains {f2(TR["mountains"]["anchor"]["rounds"][-1]["slope"])}, rivers {f2(TR["rivers"]["anchor"]["rounds"][-1]["slope"])}) and compresses where the truth spans more (companies {f2(TR["companies"]["anchor"]["rounds"][-1]["slope"])}): match the ladder to the range.</p>
<p class="so"><b>So:</b> one rating round on 24-item windows tells you the domain’s ceiling for about a tenth of a cent; a ceiling under .85 is where a stronger model or a richer state (facts in the text, not a bare label) belongs.</p>

<h2>8 · The same question asked eight ways</h2>
<figure>{wording_fig()}<figcaption>One rating round per wording, k = 24, on its own random windows; the dot is its Spearman with the canonical wording (attribute definition plus “where does x stand”) fitted from a different window draw, so wording and window both move. The black tick is the canonical wording against itself across two window draws — the floor a wording must reach to be “the same question”. Countries $ {WD["countries"]["_"]["dollars"]:.3f}, names ${WD["names"]["_"]["dollars"]:.3f}.</figcaption></figure>
<figure>{wording_table()}<figcaption><i>same windows</i>: mean ρ of the six positive rewordings with the canonical wording on identical windows (only the words move). <i>fresh windows</i>: the same on different windows. <i>one wording</i>: canonical against canonical across window draws. Self is Spearman–Brown split-half of an eight-round pool: 8w = eight wordings on eight window draws, 7w+ = the seven positive ones, 8r = eight rounds of the canonical wording — equal cost per column. ρ: the pooled sort against the reference.</figcaption></figure>
<p>At temperature zero re-asking is not a test (§ 5), so the perturbation that matters is the wording. On a fact it barely matters: every positive rewording of “population” agrees with the canonical one at {f2(min(wd("countries", "population")["same_window"][w] for w in WORDS if w != "negated"))}+ on the same windows and {f2(min(wd("countries", "population")["fresh_window"][w] for w in WORDS if w != "negated"))}–{f2(max(wd("countries", "population")["fresh_window"][w] for w in WORDS if w != "negated"))} on fresh ones, which is the same-wording floor of {f2(np.mean(wd("countries", "population")["same_wording"]))}: the variance is in which items share a call, not in the words. Eight wordings pooled reach the truth at {f2(wd("countries", "population")["pooled_diverse_pos_vs_ref"])}, eight rounds of one wording at {f2(wd("countries", "population")["pooled_repeated_vs_ref"])}. On the soft attributes the words are the dominant source of variance. Rewordings of “VC respectability” agree with the canonical one at {f2(wd("names", "vc")["fresh_window"]["persona"])} (a partner skimming a deal list) to {f2(wd("names", "vc")["fresh_window"]["formal"])} (the formal register); “rounds raised” at {f2(min(wd("names", "rounds")["fresh_window"][w] for w in WORDS if w != "negated"))}–{f2(max(wd("names", "rounds")["fresh_window"][w] for w in WORDS if w != "negated"))}. A pool of eight wordings self-agrees at {f3(min(wd("names", a)["self_diverse"] for a in ("vc", "rounds", "aura", "gay")))}–{f3(max(wd("names", a)["self_diverse"] for a in ("vc", "rounds", "aura", "gay")))} where eight rounds of one wording self-agree at {f3(min(wd("names", a)["self_repeated"] for a in ("vc", "rounds", "aura", "gay")))}+: the repeated number is the reliability of a sentence, the pooled number is the reliability of the concept, and on soft attributes they differ by a tenth.</p>
<p>Two wordings fail outright. <b>Negation is not read.</b> “How little aura does x have (highest = the least)” sorts as aura, raw ρ {f2(-wd("names", "aura")["same_window"]["negated"])} with the positive question on the same windows; “how few people live in x” is half-reversed ({f2(-wd("countries", "population")["same_window"]["negated"])}); only “how straight does x sound” follows the flip ({f2(-wd("names", "gay")["same_window"]["negated"])}, and that is a different noun, not a negation). Jev reads the attribute noun and the ladder, not the polarity of the sentence around it, so a polarity-flipped wording is a second attribute, never a check. <b>A persona rewrites the question.</b> “A partner at a top venture firm … where does x land for her” shares {f2(wd("names", "vc")["fresh_window"]["persona"])} with the definition; a casting director ranking by presence shares {f2(wd("names", "aura")["fresh_window"]["persona"])} with “aura”. The scene replaces the construct.</p>
<p class="so"><b>So:</b> diverse wordings are a validity probe, not a way to buy accuracy — run three or four positive rewordings once and read their agreement as the reliability of the concept; then spend the rounds on the one wording that carries the definition. Never flip polarity, never stage a persona.</p>

<h2>9 · Soft attributes of names: how far Jev is from a frontier judge</h2>
<figure>{names_table()}<figcaption>169 first names, four perceived attributes from the name alone. Reference: two independent Fable 5.1 reads (own shuffles), z-scored and averaged; their agreement is the ceiling any judge can reach. Cells: ρ of one Jev rating round (k = 24) under each wording against that reference.</figcaption></figure>
<p>These are the high-dimensional questions with no fact behind them, where a name is sorted by what a reader projects onto it. Two Fable reads agree at {f2(min(REFN["replica_agreement"].values()))}–{f2(max(REFN["replica_agreement"].values()))}, so the perception is stable enough to be a reference. Jev under the canonical wording reaches {f2(wd("names", "aura")["vs_ref"]["canonical"])} on aura and {f2(wd("names", "vc")["vs_ref"]["canonical"])} on VC respectability, {f2(wd("names", "gay")["vs_ref"]["canonical"])} on sounds-gay and {f2(wd("names", "rounds")["vs_ref"]["canonical"])} on rounds raised, and eight rounds lift those to {f2(wd("names", "aura")["pooled_repeated_vs_ref"])}, {f2(wd("names", "vc")["pooled_repeated_vs_ref"])}, {f2(wd("names", "gay")["pooled_repeated_vs_ref"])}, {f2(wd("names", "rounds")["pooled_repeated_vs_ref"])} while its self-agreement sits at .97: the § 6 pattern, a confident opinion that is only partly the frontier model’s. The wording that carries the definition is the one that tracks Fable — canonical and formal lead on every attribute; the short casual and plain forms lose a tenth to a third; pooling eight wordings does not close the gap ({f2(wd("names", "vc")["pooled_diverse_vs_ref"])} against {f2(wd("names", "vc")["pooled_repeated_vs_ref"])} on VC), because the gap is what Jev perceives, not how it is asked. Aura is the attribute Jev shares with Fable most, rounds raised least — a two-step inference (name → founder → funding history) that a 0.6B judge does not make.</p>
<p class="so"><b>So:</b> soft social attributes of bare names sit at a ceiling of .6–.75 against a frontier judge, below the .85 line of § 7: this is the class that escalates whole, and where the pilot-against-Fable of § 6 pays for itself in one round.</p>

<h2>10 · The whole judgment space on one grid</h2>
<figure><div class="two">{grid_fig()}</div><figcaption>{len(G.RECIPES)} recipes × four cohorts (three facts, one soft attribute against two Fable reads), six rounds each, same fit, every round priced. Thick black: <b>{WIN}</b>. <span style="color:#2f6f73">●</span> setwise rating (3/5/7/10 levels, k 4–48, plain-line format) · <span style="color:#7fb3b5">●</span> per-item yes/no (“in the top half”) · <span style="color:#c9a227">●</span> choice of one (which is highest; highest and lowest) · <span style="color:#6b5ca5">●</span> three at a time (a triple question inside a window; whole three-item states) · <span style="color:#b4552d">●</span> pairwise yes/no (all pairs, a cycle, two-item states) · <span style="color:#8a2f2f">●</span> pairwise ratio (nine-rung all pairs; nine-, five- and wide-rung cycles; two-item states) · <span style="color:#3f7d3a">●</span> anchored wide ratio. Grid total {usd(sum(GRID[c][r]["rounds"][-1]["dollars"] for c in GRID for r in GRID[c]))}.</figcaption></figure>
<figure>{grid_table()}<figcaption>Per cohort: ρ against the reference at a fixed spend of $.005 (log-interpolated between rounds; — where the first round already costs more) / after six rounds. <i>mean</i>: the $.005 composite over the four cohorts. <i>worst gap</i>: the largest amount by which any other recipe beats this one, on any cohort, at $.002, $.005, $.01 or the final round — a recipe with a small worst gap is never far from the best, whatever the budget or the question. Self is the mean six-round split-half; slope is on countries.</figcaption></figure>
<p>This is the whole space asked at once, on the same items, priced the same way: how many things a judgment looks at (one item rated among k; two items, in a two-item state or as a pair inside a window; three items; the whole window as a choice), what kind of answer it gives (a yes/no, a level out of 3, 5, 7 or 10, a ratio on a nine-rung, five-rung or 1/100…100 ladder, the one option that is highest), how many items share a state (2, 3, 4, 8, 24, 48) and how they are laid out (labelled blocks, plain numbered lines). <b>The winner is {WIN}: 24 items in a labelled state, one ten-level standing question per item, random windows, repeated.</b> Composite ρ {f3(grid_score(WIN))} at half a cent; and the largest amount any of the other {len(G.RECIPES) - 1} recipes beats it by, on any cohort at any budget, is {f3(deficit(WIN)[0])} ({deficit(WIN)[1]} on {deficit(WIN)[2]} at ${deficit(WIN)[3]}), inside the .03 run-to-run wobble of the judge. Its runners-up are its own variants — 48-item windows (worst gap {f3(deficit("rate-L10-k48")[0])}), five levels ({f3(deficit("rate-L5-k24")[0])}), three levels at k = 8 ({f3(deficit("rate-L3")[0])}) — and the first recipe from another family, choice of highest and lowest at k = 8, has a worst gap of {f3(deficit("topbot")[0])}. Nothing below the rating family is ever the best by more than the wobble, and on the soft attribute every other family loses outright.</p>
<p><b>Level count is ordinally irrelevant and cardinally decisive.</b> Three, five, seven and ten levels at k = 24 land within .01 of each other on every fact; the fitted slope against log population goes {f2(GRID["countries"]["rate-L3-k24"]["rounds"][-1]["slope"])} → {f2(GRID["countries"]["rate-L5-k24"]["rounds"][-1]["slope"])} → {f2(GRID["countries"]["rate-L7"]["rounds"][-1]["slope"])} → {f2(GRID["countries"][WIN]["rounds"][-1]["slope"])}, so ten levels are free precision. <b>Window size:</b> k = 4 starts a tenth behind (ρ {f2(gval("countries", "rate-L10-k4", .002))} on countries and {f2(gval("names-aura", "rate-L10-k4", .002))} on names at $.002 against {f2(gval("countries", WIN, .002))} and {f2(gval("names-aura", WIN, .002))}) because each call carries only three comparators; 8 is a hundredth behind 24 at the same spend; 48 buys nothing over 24. <b>Format:</b> plain numbered lines equal labelled blocks on facts and lose a tenth on bare names ({f2(GRID["names-aura"]["rate-L10-fmt"]["rounds"][-1]["rho"])} against {f2(GRID["names-aura"][WIN]["rounds"][-1]["rho"])}) — a name needs a label saying it is a name.</p>
<p><b>Per-item yes/no</b> (“x is in the top half of those shown”) is the cheapest question that works on facts — countries ρ {f3(GRID["countries"]["tophalf"]["rounds"][-1]["rho"])} for {usd(GRID["countries"]["tophalf"]["rounds"][-1]["dollars"])}, at {usd(GRID["countries"]["tophalf"]["rounds"][0]["dollars"])} a round — and loses on names ({f2(GRID["names-aura"]["tophalf"]["rounds"][-1]["rho"])}): a binary answer discards the within-half order, which on a soft attribute is most of the signal. <b>Choice of one</b> collapses with k: “which is highest” at k = 8 tops out at {f2(GRID["countries"]["top"]["rounds"][-1]["rho"])} on countries with self-agreement {f2(GRID["countries"]["top"]["rounds"][-1]["self"])}, and at k = 24 falls to {f2(GRID["countries"]["top-k24"]["rounds"][-1]["rho"])}–{f2(GRID["films"]["top-k24"]["rounds"][-1]["rho"])}, because one probability mass over 24 options is one fact per call however it is split; adding “which is lowest” rescues k = 8 (composite {f3(grid_score("topbot"))}, at {usd(GRID["countries"]["topbot"]["rounds"][0]["dollars"])} a round the best cheap recipe on facts) and not k = 24. <b>Three at a time</b> equals the rating when the triple sits inside a 24-item window ({f3(grid_score("tri"))}) and pays for the missing context when the state is only three items: <i>tri3</i> reaches the same final ρ on facts but starts at {f2(gval("countries", "tri3", .002))} on countries and {f2(gval("names-aura", "tri3", .002))} on names at $.002, sixty-six calls a round each seeing nothing but its three.</p>
<p><b>Pairwise never wins, and on the soft attribute it is the wrong instrument.</b> On facts the all-pairs ratio ladder reaches the highest final ρ of anything ({f3(GRID["countries"]["score9"]["rounds"][-1]["rho"])} on countries, {f3(GRID["films"]["score9"]["rounds"][-1]["rho"])} on films) for {usd(GRID["countries"]["score9"]["rounds"][-1]["dollars"])}, eight times the rating’s six rounds, and by $.005 every pairwise form (yes/no all pairs, yes/no cycle, ratio cycles at nine, five and wide rungs) is where the rating already was at $.002. On names the ceiling itself drops: yes/no all pairs {f2(GRID["names-aura"]["noul"]["rounds"][-1]["rho"])}, the ratio cycle {f2(GRID["names-aura"]["chain"]["rounds"][-1]["rho"])}, the wide cycle {f2(GRID["names-aura"]["widecyc"]["rounds"][-1]["rho"])}, anchored {f2(GRID["names-aura"]["anchor"]["rounds"][-1]["rho"])}, against the rating’s {f2(GRID["names-aura"][WIN]["rounds"][-1]["rho"])} — a question about x against y on a bare label draws on the thinnest thing Jev has, while a rating of 24 names in one state has the whole distribution in view and the anchors of an anchored ladder, pinned from a pilot on a soft attribute, pin noise. Two-item states are the worst use of a dollar in the grid (yes/no {f2(gval("countries", "pair2", .002))} on countries and {f2(gval("names-aura", "pair2", .002))} on names at $.002; the ratio form {f2(gval("countries", "pair2r", .005))} and {f2(gval("names-aura", "pair2r", .005))} at $.005): every call pays the instruction for one comparison.</p>
<p>Nor does what is done with the winner’s answers matter. Replaying its traces offline (<code>refit.py</code>) and reading the top level or a pooled-threshold probit latent instead of the PMF expectation, or fitting with PMF-variance weights, Huber IRLS, or slot fixed effects, moves ρ by at most .007 on any cohort (names with the top level +.026, with self-agreement down .02). The window has a real position effect — the first slot is rated up to half a level high (companies +.52 levels, se .05; the trend is −.002 to −.016 levels a slot) — and random windows absorb it: a slot fixed effect costs .03–.07 at one round, where each item has sat in one slot, and returns at most .002 after two.</p>
<p class="so"><b>So:</b> the unambiguous best sorting system on Jev is the plainest one, and the grid closes the question rather than leaving it to taste — no pairwise, triple, choice or binary design beats a 24-item ten-level rating by more than the judge’s own wobble anywhere, and every one of them loses by .05–.35 somewhere. Spend the design effort on the state (labelled, definition-carrying, 24 items) and on the attribute-level escalation of § 6, not on the question form.</p>

<h2>Recipes llmsort should endorse</h2>
<ol>
<li><b>Ordinal sort, any size — the default:</b> random windows of 24 items in labelled blocks, one ten-level standing question per item, window fixed effect in the fit, repeated. Composite ρ {f3(grid_score(WIN))} at $.005 over the § 10 grid and never more than {f3(deficit(WIN)[0])} behind anything; ρ ≥ .97 on 198 countries at {usd(rc["rate-k24"][0])}; {usd(ra["rate"][0])} to self-agreement .95 on 150 abstracts. Three levels sort as well as ten; ten are free cardinal precision.</li>
<li><b>When a per-pair read is wanted</b> (a specific comparison must be defensible, or the criterion may be unstated): yes/no on all ordered pairs in a window of 8, logit fit. Same ceiling on facts at {rc["noul"][0] / rc["rate"][0]:.1f}× the price; a tenth worse on soft attributes, so never there.</li>
<li><b>Cardinal answer:</b> three pinned anchors from a one-round rating pilot, wide ladder spanning the true range, every target against every anchor. Highest self-agreement, slope {f2(slopes["anchor"])} against truth.</li>
<li><b>Weak-knowledge domains:</b> anchored wide ratio, one round, ladder matched to the range of the truth — the best first-round number wherever the ceiling is under .9.</li>
<li><b>Escalation:</b> pilot each new attribute on ~24 items against a frontier judge; keep Jev where it agrees, hand the attribute (not individual items) to the stronger model where it does not.</li>
<li><b>Wording:</b> one wording that states the attribute definition, repeated on fresh windows; three or four positive rewordings once as a validity probe (their agreement is the reliability of the concept); no negated forms, no personas.</li>
<li><b>Do not spend on:</b> all-pairs ratio ladders for an ordinal answer ({ratio_x:.0f}× the price for the same rank information); two- or three-item states (the instruction is paid per comparison); choice-of-one questions over more than eight options; plain-line formats for bare labels; any pairwise form on a soft attribute; adaptive windows before the random plateau is measured; re-asking for freshness; pooling diverse wordings for accuracy.</li>
</ol>

<h2>Every number</h2>
<figure>{table()}<figcaption>Questions per call at the recipe’s k. Bits per item: countries against true log population (Pearson); abstracts from self-agreement, an upper bound. Threshold: countries ρ ≥ .97, abstracts self ≥ .95. Slope in nats of fitted log-ratio per nat of true log population, rating recipes omitted.</figcaption></figure>

<h2>Replay</h2>
<p class="foot">Every Jev response is cached by request hash in <code>trace-&lt;cohort&gt;.jsonl.gz</code> and <code>trace-drift-&lt;cohort&gt;.jsonl.gz</code>; <code>lab.py &lt;cohort&gt; &lt;recipe&gt; [rounds] [k]</code>, <code>drift.py</code> and <code>report.py</code> then run with no key. <code>run_countries.sh</code>, <code>run_arxiv.sh</code>, <code>run_more.sh</code>, <code>run_truth.sh</code> are the batteries; <code>cohorts.py</code> fetches the Wikidata cohorts; <code>cascade.py</code> is § 6; <code>wordings.py</code> is §§ 8–9 (<code>trace-wordings-&lt;cohort&gt;.jsonl.gz</code>, <code>ref/names-r{{0,1}}.json</code> the two Fable reads); <code>grid.py &lt;cohort&gt; [recipe …]</code> is § 10 (<code>run_grid.sh</code>, <code>run_grid2.sh</code>, <code>trace-grid-&lt;cohort&gt;.jsonl.gz</code>, <code>grid-&lt;cohort&gt;-&lt;recipe&gt;.json</code>); <code>refit.py</code> replays the winner’s traces under the other readouts and fits (<code>refit.json</code>). Limits: one judge; one wording per question form except in § 8; the abstracts reference is a 31B generating judge on 40 items, so ρ there is bounded by its reliability and self-agreement carries the comparison.</p>
</html>"""
open(f"{HERE}/report.html", "w").write(HTML)
print("report.html", len(HTML), f"${TOTAL:.2f}")
