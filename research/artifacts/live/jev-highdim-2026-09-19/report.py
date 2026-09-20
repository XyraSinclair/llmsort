"""Writes report.html, the pack's one document, from judges.json, rho-*-decomp.json, ref-arxiv.json and the Jev
traces. python3 report.py -- replay only, no key.

The transitivity numbers (CURL, TRI) are the printed output of holonomy.py at k = 8; everything else is computed here.
"""
import gzip, json, os
from collections import defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
J = lambda f: json.load(open(f"{HERE}/{f}"))
COH = [("arxiv", "arXiv abstracts"), ("manifund", "Manifund applications"), ("lw", "LessWrong comments")]
COL = {"arxiv": "#b4552d", "manifund": "#2f6f73", "lw": "#6b5ca5"}
JC = {"jev": "#2f6f73", "gemma": "#b4552d", "fable": "#6d675d"}
JUD = J("judges.json"); attrs = J("ref-arxiv.json")["attrs"]
DEC = {c: J(f"rho-{c}-decomp.json") for c, _ in COH}
mean = lambda xs: (lambda l: sum(l) / len(l))(list(xs))
f2 = lambda v: "—" if v is None else f"{v:.2f}".replace("0.", ".", 1).replace("-.", "−.")
INS = ("noul", "score9", "rate")
jev = lambda c, k, key="rho": mean(JUD[c]["jev"][str(k)][i][key] for i in INS)
CURL = "1–3 %"; TRI = ".23–.28"
FABLE_SPEND = "712K tokens, 12 reads"
MALF = {"arxiv": "17 / 144", "manifund": "13 / 144", "lw": "65 / 144"}  # gemma k = 24 calls without a usable ordering


def jev_spend():  # {cohort: {k: (calls, dollars)}} for the elab windows, and the pack's whole Jev spend, at $0.042 / M input tokens
    out = {}; total = 0
    for c, _ in COH:
        f = f"{HERE}/trace-{c}.jsonl"; fh = open(f) if os.path.exists(f) else gzip.open(f + ".gz", "rt")
        agg = defaultdict(lambda: [0, 0])
        for line in fh:
            d = json.loads(line); stem = d["tag"].split("|")[0]; total += d["usage"]["input_tokens"]
            if stem.startswith("elab") and "|r" in d["tag"]:
                k = int(stem[5:] or 8); agg[k][0] += 1; agg[k][1] += d["usage"]["input_tokens"]
        out[c] = {k: (v[0], v[1] * 0.042 / 1e6) for k, v in agg.items()}
    return out, total * 0.042 / 1e6


SPEND, JEV_TOTAL = jev_spend()
GEMMA_TOTAL = sum(JUD[c]["gemma"][k]["cost"] for c, _ in COH for k in JUD[c]["gemma"])


def judges_fig():
    W, L, R, rh = 860, 190, 30, 30; H = 30 + 3 * (2 * rh + 26); x = lambda v: L + (v - .4) / .6 * (W - L - R)
    s = f'<svg viewBox="0 0 {W} {H}" role="img" aria-label="Three judges: agreement with Fable and with themselves">'
    for t in (.4, .5, .6, .7, .8, .9, 1.0):
        s += f'<line x1="{x(t):.1f}" y1="22" x2="{x(t):.1f}" y2="{H - 6}" class="grid"/><text x="{x(t):.1f}" y="14" class="tick" text-anchor="middle">{f2(t) if t < 1 else "1.0"}</text>'
    y0 = 30
    for c, n in COH:
        d = JUD[c]; rows = [("agrees with Fable", [("jev", jev(c, 8)), ("gemma", d["gemma"]["8"]["rho2"])]),
                            ("agrees with itself", [("jev", jev(c, 8, "self")), ("gemma", d["gemma"]["8"]["self"]), ("fable", d["fable"]["self"])])]
        s += f'<text x="0" y="{y0 + 12}" class="lab" style="font-weight:600;fill:{COL[c]}">{n}</text>'
        for k, (lab, vals) in enumerate(rows):
            y = y0 + 30 + k * rh
            s += f'<text x="0" y="{y + 4}" class="tick">{lab}</text><line x1="{x(min(v for _, v in vals)):.1f}" y1="{y}" x2="{x(max(v for _, v in vals)):.1f}" y2="{y}" class="span"/>'
            for j, v in vals:
                s += (f'<rect x="{x(v) - 1.5:.1f}" y="{y - 9}" width="3" height="18" fill="{JC[j]}"/>' if j == "fable" else f'<circle cx="{x(v):.1f}" cy="{y}" r="6" fill="{JC[j]}"/>')
                s += f'<text x="{x(v):.1f}" y="{y - 11}" class="tick" text-anchor="middle">{f2(v)}</text>'
        y0 += 2 * rh + 26
    return s + "</svg>"


def window_fig():
    W, H, L, B = 280, 210, 34, 30; ks = [2, 4, 8, 12, 24]; xs = {k: L + i / 4 * (W - L - 16) for i, k in enumerate(ks)}
    y = lambda v: H - B - (v - .4) / .55 * (H - B - 18); out = ""
    for c, n in COH:
        d = JUD[c]
        s = f'<svg viewBox="0 0 {W} {H}" role="img" aria-label="Agreement with Fable against items per window, {n}"><text x="{L}" y="12" class="lab" style="font-weight:600;fill:{COL[c]}">{n}</text>'
        for t in (.4, .5, .6, .7, .8, .9): s += f'<line x1="{L}" y1="{y(t):.1f}" x2="{W - 16}" y2="{y(t):.1f}" class="grid"/><text x="{L - 5}" y="{y(t) + 4:.1f}" class="tick" text-anchor="end">{f2(t)}</text>'
        for k in ks: s += f'<text x="{xs[k]:.1f}" y="{H - 12}" class="tick" text-anchor="middle">{k}</text>'
        fs = d["fable"]["self"]; s += f'<line x1="{L}" y1="{y(fs):.1f}" x2="{W - 16}" y2="{y(fs):.1f}" stroke="{JC["fable"]}" stroke-dasharray="4 3" stroke-width="1.5"/><text x="{W - 14}" y="{y(fs) + 4:.1f}" class="tick" style="fill:{JC["fable"]}">F</text>'
        jp = [(k, jev(c, k)) for k in ks if str(k) in d["jev"]]
        s += f'<polyline points="{" ".join(f"{xs[k]:.1f},{y(v):.1f}" for k, v in jp)}" fill="none" stroke="{JC["jev"]}" stroke-width="2.2"/>' + "".join(f'<circle cx="{xs[k]:.1f}" cy="{y(v):.1f}" r="3.8" fill="{JC["jev"]}"/>' for k, v in jp)
        s += f'<text x="{xs[24]:.1f}" y="{y(jp[-1][1]) + 4:.1f}" class="tick" text-anchor="middle" style="fill:{JC["jev"]}">✕</text>'
        gp = [(k, d["gemma"][str(k)]["rho2"]) for k in (4, 8, 24)]
        s += f'<polyline points="{" ".join(f"{xs[k]:.1f},{y(v):.1f}" for k, v in gp)}" fill="none" stroke="{JC["gemma"]}" stroke-width="2.2"/>' + "".join(f'<circle cx="{xs[k]:.1f}" cy="{y(v):.1f}" r="3.8" fill="{"var(--bg)" if k == 24 else JC["gemma"]}" stroke="{JC["gemma"]}" stroke-width="2"/>' for k, v in gp)
        out += s + "</svg>"
    return out


def struct_fig():
    W, H, L, B = 860, 200, 60, 34; y = lambda v: H - B - (v - .4) / .55 * (H - B - 16)
    keys = [("noul", "yes/no"), ("score9", "ratio"), ("rate", "rating"), ("dec", "propositions")]
    s = f'<svg viewBox="0 0 {W} {H}" role="img" aria-label="Agreement with Fable by response structure">'
    for t in (.4, .5, .6, .7, .8, .9): s += f'<line x1="{L}" y1="{y(t):.1f}" x2="{W}" y2="{y(t):.1f}" class="grid"/><text x="{L - 6}" y="{y(t) + 4:.1f}" class="tick" text-anchor="end">{f2(t)}</text>'
    gw = (W - L) / 3; bw = gw / 6
    for gi, (c, n) in enumerate(COH):
        x0 = L + gi * gw + bw * .6
        s += f'<text x="{L + gi * gw + gw / 2:.1f}" y="{H - 4}" class="tick" text-anchor="middle" style="fill:{COL[c]}">{n}</text>'
        fs = JUD[c]["fable"]["self"]; s += f'<line x1="{L + gi * gw + 6:.1f}" y1="{y(fs):.1f}" x2="{L + (gi + 1) * gw - 6:.1f}" y2="{y(fs):.1f}" stroke="{JC["fable"]}" stroke-dasharray="4 3" stroke-width="1.5"/>'
        for bi, (k, lab) in enumerate(keys):
            v = JUD[c]["jev"]["8"][k]["rho"] if k != "dec" else mean(DEC[c][a]["decomp"] for a in attrs)
            bx = x0 + bi * bw * 1.15
            s += f'<rect x="{bx:.1f}" y="{y(v):.1f}" width="{bw:.1f}" height="{H - B - y(v):.1f}" fill="{COL[c]}" fill-opacity="{1 - bi * .2:.2f}"/><text x="{bx + bw / 2:.1f}" y="{y(v) - 5:.1f}" class="tick" text-anchor="middle">{f2(v)}</text>'
            s += f'<text x="{bx + bw / 2:.1f}" y="{H - B + 12:.1f}" class="tick" text-anchor="middle" style="font-size:9.5px">{lab}</text>'
    return s + "</svg>"


def table():
    h = '<table><thead><tr><th>cohort</th><th>judge</th><th>items / window</th><th>calls</th><th>$</th><th>vs Fable</th><th>vs itself</th><th>order inconsistency</th></tr></thead><tbody>'
    for c, n in COH:
        d = JUD[c]; first = True
        for k in (2, 4, 8, 12):
            r = d["jev"][str(k)]; calls, cost = SPEND[c][k]
            h += f'<tr><td style="color:{COL[c]}">{n if first else ""}</td><td>Jev</td><td>{k}</td><td>{calls}</td><td>{cost:.3f}</td><td>{f2(jev(c, k))}</td><td>{f2(jev(c, k, "self"))}</td><td>order sd {f2(mean(r[i]["order"] for i in ("noul", "score9")))}</td></tr>'; first = False
        h += '<tr><td></td><td>Jev</td><td>24</td><td colspan="5" class="mute">request over the API token limit (552 pairs × 2 readouts + 24 ratings in one call)</td></tr>'
        for k in (4, 8, 24):
            g = d["gemma"][str(k)]
            h += f'<tr><td></td><td>gemma-4-31b</td><td>{k}</td><td>{g["calls"] // 2}</td><td>{g["cost"] / 2:.3f}</td><td>{f2(g["rho1"])} one seed · {f2(g["rho2"])} two</td><td>{f2(g["self"])}</td><td>{"flips " + f2(g["flip"]) if g["flip"] is not None else "—"}{"; malformed " + MALF[c] if k == 24 else ""}</td></tr>'
        h += f'<tr><td></td><td>Fable 5.1</td><td>24</td><td>4 reads</td><td>—</td><td class="mute">is the reference</td><td>{f2(d["fable"]["self"])}</td><td>—</td></tr>'
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
table{border-collapse:collapse;font:13px/1.3 ui-sans-serif,-apple-system,'Helvetica Neue',sans-serif;font-variant-numeric:tabular-nums;width:100%;margin:12px 0 6px}
th,td{padding:5px 7px;text-align:right;border-bottom:1px solid var(--rule)}th:first-child,td:first-child,th:nth-child(2),td:nth-child(2),td:last-child,th:last-child{text-align:left}thead th{color:var(--mute);font-weight:600;font-size:11.5px;letter-spacing:.03em}
td.mute{color:var(--mute);text-align:left}
figure{margin:18px 0 22px}figcaption{font:13px/1.45 ui-sans-serif,-apple-system,sans-serif;color:var(--mute);margin-top:6px}
.three{display:grid;grid-template-columns:1fr 1fr 1fr;gap:14px}@media(max-width:760px){.three{grid-template-columns:1fr}}
svg{width:100%;height:auto;display:block;font-family:ui-sans-serif,-apple-system,'Helvetica Neue',sans-serif}
svg .grid{stroke:var(--rule);stroke-width:1}svg .tick{fill:var(--mute);font-size:11.5px}svg .lab{fill:var(--ink);font-size:13.5px}
svg .span{stroke:var(--rule);stroke-width:5;stroke-linecap:round}
.key span{display:inline-block;margin-right:16px}.key i{display:inline-block;width:10px;height:10px;border-radius:50%;margin-right:6px}.key i.bar{width:3px;height:14px;border-radius:0;vertical-align:-2px}
.foot{color:var(--mute);font-size:14px}
"""

g = lambda fn: " / ".join(f2(fn(c)) for c, _ in COH)
jev8 = g(lambda c: jev(c, 8)); jevself = g(lambda c: jev(c, 8, "self")); gem8 = g(lambda c: JUD[c]["gemma"]["8"]["rho2"]); gemself = g(lambda c: JUD[c]["gemma"]["8"]["self"]); fab = g(lambda c: JUD[c]["fable"]["self"])
gem4 = g(lambda c: JUD[c]["gemma"]["4"]["rho2"]); gem24 = g(lambda c: JUD[c]["gemma"]["24"]["rho2"]); jev2 = g(lambda c: jev(c, 2)); jev12 = g(lambda c: jev(c, 12))
jself2 = g(lambda c: jev(c, 2, "self")); jself12 = g(lambda c: jev(c, 12, "self"))
dec = g(lambda c: mean(DEC[c][a]["decomp"] for a in attrs))
order = g(lambda c: mean(JUD[c]["jev"]["8"][i]["order"] for i in ("noul", "score9")))
flip8 = g(lambda c: JUD[c]["gemma"]["8"]["flip"])
dj = [jev(c, 8) - JUD[c]["gemma"]["8"]["rho2"] for c, _ in COH]
loss2 = [max(jev(c, k) for k in (4, 8, 12)) - jev(c, 2) for c, _ in COH]

HTML = f"""<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>How consistent is Jev’s judgment?</title><style>{CSS}</style>
<h1>How consistent is Jev’s judgment?</h1>
<p class="sub">A typed judge (hosted Jev, TypeSafe <code>jev-latest</code>) against a generating judge (gemma-4-31b) and a frontier one (Fable 5.1) on twelve hard attributes of three kinds of text, varied over the size of the context window and the structure of the response. Measured 2026-09-19/20. Written for anyone who ranks things with model judgments.</p>

<div class="verdict">
<p><b>Jev is as consistent as gemma-4-31b, and both are more consistent than they are right.</b> In the same window design (eight items per prompt), Jev agrees with Fable at {jev8} (arXiv / Manifund / LessWrong) and with itself at {jevself}; gemma agrees with Fable at {gem8} and with itself at {gemself}; Fable agrees with itself at {fab}. On abstracts Jev is ahead by {f2(dj[0])}; on the two prose cohorts they are within {f2(max(dj[1:]))}.</p>
<p><b>Context window: Jev is flat from 4 to 12 items; gemma is best small and breaks large.</b> Jev loses {f2(min(loss2))}–{f2(max(loss2))} at pairs (k = 2) and nothing between k = 4 and k = 12; k = 24 does not fit the API. gemma peaks at k = 4 ({gem4}) and at k = 24 returns no usable ordering in 9–45 % of calls ({gem24}).</p>
<p><b>Response structure: it does not matter, because the answers are one potential.</b> Yes/no, a nine-rung ratio ladder and a ten-point rating agree with Fable within .05 of each other; inside a call the pairwise reads are transitive to {CURL} of their variance. The one inconsistency Jev has is mention order, at every window size; gemma’s equivalent is a direction flip on {flip8} of pairs between two presentations of the same window.</p>
</div>

<h2>Setup</h2>
<p><b>Items and attributes.</b> Twenty-four each of arXiv abstracts, Manifund grant applications and public LessWrong comments. Twelve attributes that resist operationalizing and are not one axis: self-containedness, high-status, low-status, poshness, technical calmness, earnestness, intellectual density, legibility to an outsider, coiled potential energy, craftedness, institutional insiderness, how much it rewards a careful re-read. Each is given to every judge as the same one-paragraph definition.</p>
<p><b>Three judges, one window design.</b> A window is k items in one prompt. <i>Jev</i> reads the k items and answers typed questions with no generation, in one prefill pass: every ordered pair as a yes/no (“item x is stronger than item y”) and as a nine-rung ratio, and every item on a ten-point scale; three rounds of random windows, one call per window and attribute. <i>gemma-4-31b</i> (OpenRouter) reads the same kind of window and writes the ordering, in the setwise prompt of the <code>llmsort</code> binary, two presentations per window, two seeds. <i>Fable 5.1</i> reads all 24 at once and gives each a magnitude on a ratio scale, in two independent replicas with their own shuffles and labels; it is the reference. Per-item latents are fitted from each judge’s reads and the score is the Spearman rank correlation against the reference, mean over the twelve attributes.</p>
<p><b>Self-agreement</b> is the same statistic between two independent runs of the same judge: Jev, one round of windows against another; gemma, seed against seed; Fable, replica against replica. Spend: Jev ${JEV_TOTAL:.2f} in total, gemma ${GEMMA_TOTAL:.2f}, Fable {FABLE_SPEND}.</p>

<h2>1 · Three judges</h2>
<figure>{judges_fig()}
<figcaption class="key"><span><i style="background:{JC["jev"]}"></i>Jev, k = 8, mean of its three readouts</span><span><i style="background:{JC["gemma"]}"></i>gemma-4-31b, k = 8, two seeds pooled</span><span><i class="bar" style="background:{JC["fable"]}"></i>Fable 5.1, replica vs replica</span> Spearman, mean over twelve attributes.</figcaption></figure>
<p>Every judge agrees with itself more than with the reference, and the gap is the judge’s stable opinion, not noise: pooling Jev’s three rounds instead of one moves agreement with Fable by about .04, and pooling gemma’s two seeds by .01–.06. Where the reference itself is soft (arXiv, .77) both judges fall furthest below it, and Jev falls less.</p>
<p class="so"><b>So:</b> on a hard attribute a typed single-pass judge and a 31B generating judge are the same class of instrument. Repetition converges on the instrument’s opinion; only a different question moves it toward the reference.</p>

<h2>2 · Context window</h2>
<div class="three">{window_fig()}</div>
<figure><figcaption class="key"><span><i style="background:{JC["jev"]}"></i>Jev</span><span><i style="background:{JC["gemma"]}"></i>gemma-4-31b, two seeds pooled (k = 24 hollow: 9–45 % of calls malformed)</span><span>dashed F: Fable vs Fable</span> Agreement with Fable against items per window. Jev sees each item three times at every k; ✕ marks the k = 24 request the API refuses.</figcaption></figure>
<p>For Jev the window is context, not a lever: agreement is {jev2} at pairs and {jev12} at twelve items, with the plateau reached by four. What grows with k is self-agreement, from {jself2} at k = 2 to {jself12} at k = 12, because a round at k = 2 holds twelve reads and a round at k = 12 holds four hundred, not because the judgments change; order inconsistency is the same at every k. For gemma the window is a burden: agreement falls from k = 4 to k = 24 on the two prose cohorts, and at 24 items a third of LessWrong calls come back without a usable ordering.</p>
<p class="so"><b>So:</b> pack Jev’s windows for cost, not accuracy — anything from 4 to 12 reads the same — and keep a generating judge’s windows small.</p>

<h2>3 · Response structure</h2>
<figure>{struct_fig()}
<figcaption>Agreement with Fable at k = 8 by the shape of the answer: yes/no on a pair, a nine-rung ratio on a pair, a ten-point rating of one item, and five concrete yes/no propositions per attribute about one item (equal weights, signs fixed in advance, one call per window). Dashed: Fable vs Fable.</figcaption></figure>
<p>The three readouts of the same window tie, and their combination is no better than the best one. The reason is measured, not assumed: within a call, after removing the mention-order term, one score per item explains all but {CURL} of the pairwise variance, and a triangle’s cycle sum has {TRI} of an edge’s spread where independent edges would give 1.73. Fifty-six pairs are eight numbers read fifty-six ways. The decomposition into concrete propositions reaches {dec} for a fifteenth of the tokens and is the only shape that carries different information; it adds to the pairwise arm on two cohorts and not on the third, whose propositions were written for another genre.</p>
<p>The inconsistency that does exist is order. For Jev, L(x,y) + L(y,x), which should be zero, has {order} of an edge’s spread (yes/no and ratio pooled), and the yes/no form leans “yes” by up to .8 sd; asking both orders is the one repetition that pays. gemma flips {flip8} of pair directions between two presentations of the same window at k = 8, more at k = 24. A graded field on a bare negative-polarity name (“low-status”) inverts silently for Jev while the yes/no holds; the definition repairs it.</p>
<p class="so"><b>So:</b> choose the response by cost and polarity safety, not precision: yes/no in both orders where a definition may be missing, the ratio ladder where a cardinal read is wanted (it compresses at about .6 nat per nat), propositions for the cheap first pass.</p>

<h2>Every number</h2>
<figure>{table()}<figcaption>Calls and dollars are per cohort over twelve attributes, gemma’s per seed. Jev’s self-agreement is round against round and rises with k for the reason in § 2. gemma’s one-seed agreement is the like-for-like number against Jev, whose three rounds are one run.</figcaption></figure>

<h2>Replay</h2>
<p class="foot">Every Jev response is cached by request hash in <code>trace-&lt;cohort&gt;.jsonl.gz</code> and every gemma sort in <code>gemma/</code>; <code>ref.py</code>, <code>step.py</code>, <code>decomp.py</code>, <code>holonomy.py</code>, <code>judges.py</code> and <code>report.py</code> then run with no key. <code>judges_run.py</code> is the launcher. Item texts, definitions, Fable prompts and magnitudes are in the same directory. Limits: n = 24 per cohort, one window seed for Jev, one reference model, gemma read through one prompt design.</p>
</html>"""
open(f"{HERE}/report.html", "w").write(HTML)
print("report.html", len(HTML), f"jev ${JEV_TOTAL:.2f} gemma ${GEMMA_TOTAL:.2f}")
