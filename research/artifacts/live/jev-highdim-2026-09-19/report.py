"""Writes report.html from the pack's result files (rho-*.json, ref-*.json). python3 report.py -- no key, no network.

Per-attribute numbers and means are read from the JSON; the holonomy, round-to-round, pruning and proposition-count
numbers are the printed output of holonomy.py, prune.py and props_curve.py, and the jev-bits numbers are from
../jev-bits-2026-09-19/RESULTS.md.
"""
import json, os

HERE = os.path.dirname(os.path.abspath(__file__))
J = lambda f: json.load(open(f"{HERE}/{f}")) if os.path.exists(f"{HERE}/{f}") else None
COH = [("arxiv", "arXiv abstracts"), ("manifund", "Manifund applications"), ("lw", "LessWrong comments")]
COL = {"arxiv": "#b4552d", "manifund": "#2f6f73", "lw": "#6b5ca5"}
attrs = J("ref-arxiv.json")["attrs"]
SHORT = {"how much it rewards a careful re-read": "rewards a re-read", "legibility to an outsider": "legibility to outsider"}
D = {c: {"ref": J(f"ref-{c}.json"), "bare": J(f"rho-{c}-bare.json"), "elab": J(f"rho-{c}-elab.json"),
         "dec": J(f"rho-{c}-decomp.json"), "dec10": J(f"rho-{c}-decomp10.json")} for c, _ in COH}
mean = lambda xs: sum(xs) / len(xs)
f2 = lambda v: "—" if v is None else (f"{v:.2f}".replace("0.", ".", 1) if abs(v) < 1 else f"{v:.2f}").replace("-.", "−.")


def M(c, src, key):
    return None if D[c][src] is None else mean([D[c][src][a][key] for a in attrs])


MEANS = {c: {"ceiling": mean([D[c]["ref"]["reliability"][a][0] for a in attrs]), "bare": M(c, "bare", "all"),
             "noul": M(c, "elab", "noul"), "score9": M(c, "elab", "score9"), "rate": M(c, "elab", "rate"),
             "elab": M(c, "elab", "all"), "decomp": M(c, "dec", "decomp"), "both": M(c, "dec", "both"),
             "decomp10": M(c, "dec10", "decomp")} for c, _ in COH}
PRUNED = {"arxiv": .67, "manifund": .68, "lw": .61}        # prune.py
CURVE = {"arxiv": [.37, .47, .53, .58, .62], "manifund": [.47, .57, .62, .65, .67], "lw": [.41, .52, .58, .62, .65]}  # props_curve.py
HOL = [("arxiv", "score9", .57, -.20, .03, .28), ("arxiv", "noul", .73, .35, .02, .24), ("manifund", "score9", .45, .00, .02, .25),
       ("manifund", "noul", .48, .67, .02, .23), ("lw", "score9", .56, -.29, .02, .24), ("lw", "noul", .57, .80, .01, .23)]  # holonomy.py
SELF = {"arxiv": (.81, .64), "manifund": (.85, .65), "lw": (.84, .70)}  # score9: round vs round, one round vs Fable


def cell(v, ceiling=False):
    if v is None: return '<td class="na">—</td>'
    if v < 0: bg, fg = f"rgba(168,52,32,{min(1, .25 + abs(v)):.2f})", "#fff"
    else:
        t = max(0, min(1, (v - .2) / .78)); bg = f"rgba({47 if not ceiling else 70},{111 if not ceiling else 70},{115 if not ceiling else 70},{.06 + .9 * t:.2f})"; fg = "#fff" if t > .55 else "var(--ink)"
    return f'<td style="background:{bg};color:{fg}">{f2(v)}</td>'


def heat():
    h = '<table class="heat"><thead><tr><th></th>' + "".join(f'<th colspan="5" style="color:{COL[c]}">{n}</th>' for c, n in COH) + "</tr><tr><th></th>"
    h += "".join('<th>name</th><th>defn</th><th>props</th><th>both</th><th class="cl">Fable</th>' for _ in COH) + "</tr></thead><tbody>"
    for a in attrs + [None]:
        h += f'<tr{" class=mean" if a is None else ""}><th>{"mean of 12" if a is None else SHORT.get(a, a)}</th>'
        for c, _ in COH:
            d = D[c]
            if a is None: vals = [MEANS[c]["bare"], MEANS[c]["elab"], MEANS[c]["decomp"], MEANS[c]["both"], MEANS[c]["ceiling"]]
            else: vals = [d["bare"][a]["all"] if d["bare"] else None, d["elab"][a]["all"], d["dec"][a]["decomp"], d["dec"][a]["both"], d["ref"]["reliability"][a][0]]
            h += "".join(cell(v, i == 4) for i, v in enumerate(vals))
        h += "</tr>"
    return h + "</tbody></table>"


def dots():
    rows = [("bare", "attribute name only", "108 calls"), ("elab", "definition beside the question", "108 calls"), ("decomp", "5 concrete propositions per attribute", "9 calls"),
            ("both", "definition + propositions", "117 calls"), ("decomp10", "10 propositions", "9 calls"), ("pruned", "10, pruned without a reference", "9 calls"), ("ceiling", "Fable against Fable", "reference")]
    W, H, L, R, top, rh = 860, 40 + 34 * len(rows), 300, 40, 26, 34; x = lambda v: L + (v - .4) / .6 * (W - L - R)
    s = f'<svg viewBox="0 0 {W} {H}" role="img" aria-label="Mean rank agreement with the Fable reference, by step and cohort">'
    for t in (.4, .5, .6, .7, .8, .9, 1.0):
        s += f'<line x1="{x(t):.1f}" y1="{top - 6}" x2="{x(t):.1f}" y2="{H - 14}" class="grid"/><text x="{x(t):.1f}" y="{top - 11}" class="tick" text-anchor="middle">{f2(t) if t < 1 else "1.0"}</text>'
    for k, (key, lab, n) in enumerate(rows):
        y = top + rh * k + rh / 2
        s += f'<text x="0" y="{y + 4:.1f}" class="lab">{lab}</text><text x="{L - 14}" y="{y + 4:.1f}" class="tick" text-anchor="end">{n}</text>'
        vals = [(c, PRUNED[c] if key == "pruned" else MEANS[c][key]) for c, _ in COH]; vals = [(c, v) for c, v in vals if v is not None]
        s += f'<line x1="{x(min(v for _, v in vals)):.1f}" y1="{y:.1f}" x2="{x(max(v for _, v in vals)):.1f}" y2="{y:.1f}" class="span"/>'
        for c, v in vals:
            s += (f'<rect x="{x(v) - 1.5:.1f}" y="{y - 9:.1f}" width="3" height="18" fill="{COL[c]}"/>' if key == "ceiling" else f'<circle cx="{x(v):.1f}" cy="{y:.1f}" r="5.5" fill="{COL[c]}"/>')
    return s + "</svg>"


def curve():
    W, H, L, B = 400, 230, 40, 30; x = lambda k: L + (k - 1) / 4 * (W - L - 70); y = lambda v: H - B - (v - .3) / .45 * (H - B - 16)
    s = f'<svg viewBox="0 0 {W} {H}" role="img" aria-label="Agreement against number of propositions">'
    for t in (.3, .4, .5, .6, .7): s += f'<line x1="{L}" y1="{y(t):.1f}" x2="{W - 60}" y2="{y(t):.1f}" class="grid"/><text x="{L - 8}" y="{y(t) + 4:.1f}" class="tick" text-anchor="end">{f2(t)}</text>'
    for k in range(1, 6): s += f'<text x="{x(k):.1f}" y="{H - 10}" class="tick" text-anchor="middle">{k}</text>'
    for c, n in COH:
        pts = " ".join(f"{x(k + 1):.1f},{y(v):.1f}" for k, v in enumerate(CURVE[c]))
        s += f'<polyline points="{pts}" fill="none" stroke="{COL[c]}" stroke-width="2"/>' + "".join(f'<circle cx="{x(k + 1):.1f}" cy="{y(v):.1f}" r="3.5" fill="{COL[c]}"/>' for k, v in enumerate(CURVE[c]))
        s += f'<text x="{x(5) + 9:.1f}" y="{y(CURVE[c][-1]) + 4 + (8 if c == "lw" else -6 if c == "manifund" else 6):.1f}" class="tick" fill="{COL[c]}" style="fill:{COL[c]}">{c}</text>'
    return s + "</svg>"


def tri():
    W, H, L = 400, 230, 150; x = lambda v: L + v / 1.8 * (W - L - 110)
    s = f'<svg viewBox="0 0 {W} {H}" role="img" aria-label="Triangle cycle sum against an edge">'
    for k, (lab, sub, v, cls) in enumerate([("independent edges", "no latent at all", 1.73, "bar0"), ("Jev, measured", "six cohort × readout cells", .25, "bar1"), ("a perfect potential", "fully transitive", 0, "bar1")]):
        yy = 34 + k * 62
        s += f'<text x="0" y="{yy + 2}" class="lab">{lab}</text><text x="0" y="{yy + 18}" class="tick">{sub}</text><rect x="{L}" y="{yy - 12}" width="{max(2, x(v) - L):.1f}" height="26" class="{cls}"/><text x="{max(L + 2, x(v)) + 8:.1f}" y="{yy + 6}" class="num">{"√3 = 1.73" if v > 1 else ".23–.28" if v else "0"}</text>'
    return s + f'<text x="0" y="{H - 8}" class="tick">sd of L(a,b)+L(b,c)+L(c,a), in units of one edge’s sd</text></svg>'


def selfc():
    W, H, L = 400, 230, 150; x = lambda v: L + (v - .5) / .45 * (W - L - 30)
    s = f'<svg viewBox="0 0 {W} {H}" role="img" aria-label="Self-agreement against agreement with the reference">'
    for t in (.5, .6, .7, .8, .9): s += f'<line x1="{x(t):.1f}" y1="16" x2="{x(t):.1f}" y2="{H - 40}" class="grid"/><text x="{x(t):.1f}" y="{H - 26}" class="tick" text-anchor="middle">{f2(t)}</text>'
    for k, (c, n) in enumerate(COH):
        yy = 44 + k * 52; rr, rf = SELF[c]
        s += f'<text x="0" y="{yy + 4}" class="lab">{n.split()[0]}</text><line x1="{x(rf):.1f}" y1="{yy}" x2="{x(rr):.1f}" y2="{yy}" stroke="{COL[c]}" stroke-width="2"/>'
        s += f'<circle cx="{x(rf):.1f}" cy="{yy}" r="5.5" fill="var(--bg)" stroke="{COL[c]}" stroke-width="2"/><circle cx="{x(rr):.1f}" cy="{yy}" r="5.5" fill="{COL[c]}"/>'
        s += f'<text x="{x(rf):.1f}" y="{yy - 11}" class="tick" text-anchor="middle">{f2(rf)}</text><text x="{x(rr):.1f}" y="{yy - 11}" class="tick" text-anchor="middle">{f2(rr)}</text>'
    return s + f'<text x="0" y="{H - 6}" class="tick">○ one round against Fable   ● one round against another round</text></svg>'


m = MEANS; g = lambda k: " / ".join(f2(m[c][k]) for c, _ in COH)
holrows = "".join(f'<tr><td style="color:{COL[c]}">{c}</td><td>{i}</td><td>{f2(o)}</td><td>{"+" if b > 0 else ""}{f2(b)}</td><td>{f2(cu)}</td><td>{f2(t)}</td></tr>' for c, i, o, b, cu, t in HOL)
lowbare = "".join(f'<tr><td style="color:{COL[c]}">{c}</td>' + "".join(cell(D[c]["bare"]["low-status"][k]) for k in ("noul", "score9", "rate")) + "".join(cell(D[c]["elab"]["low-status"][k]) for k in ("noul", "score9", "rate")) + "</tr>" for c in ("arxiv", "manifund"))

CSS = """
:root{--bg:#faf7f1;--ink:#1c1a17;--mute:#6d675d;--rule:#d9d2c4;--card:#f2ede3;--acc:#2f6f73}
@media(prefers-color-scheme:dark){:root{--bg:#15161a;--ink:#e8e4da;--mute:#9a9487;--rule:#33353c;--card:#1d1f25;--acc:#6fb7bb}}
*{box-sizing:border-box}html{background:var(--bg);color:var(--ink);font:16px/1.55 Charter,'Iowan Old Style','Palatino Linotype',Georgia,serif;-webkit-text-size-adjust:100%}
body{margin:0 auto;padding:56px 28px 96px;max-width:920px}
h1{font-size:34px;line-height:1.15;margin:0 0 10px;letter-spacing:-.01em;font-weight:600}
h2{font-size:13px;letter-spacing:.14em;text-transform:uppercase;font-family:ui-sans-serif,-apple-system,'Helvetica Neue',sans-serif;color:var(--mute);font-weight:600;margin:56px 0 14px;padding-top:14px;border-top:1px solid var(--rule)}
h3{font-size:19px;margin:30px 0 6px;font-weight:600;line-height:1.3}h3 .n{color:var(--acc);font-variant-numeric:tabular-nums;margin-right:.5em}
p{margin:0 0 12px}.sub{color:var(--mute);font-size:15px;margin-bottom:26px}.lede{font-size:19px;line-height:1.5}
.verdict{background:var(--card);border-left:3px solid var(--acc);padding:16px 20px;margin:22px 0}.verdict p:last-child{margin:0}
.so{color:var(--mute);font-style:italic}.so b{font-style:normal;color:var(--ink);font-weight:600}
code,.mono{font:13px/1.4 ui-monospace,'SF Mono',Menlo,monospace}code{background:var(--card);padding:1px 5px;border-radius:3px}
table{border-collapse:collapse;font:13px/1.3 ui-sans-serif,-apple-system,'Helvetica Neue',sans-serif;font-variant-numeric:tabular-nums;width:100%;margin:12px 0 6px}
th,td{padding:5px 7px;text-align:right;border-bottom:1px solid var(--rule)}th:first-child,td:first-child{text-align:left}thead th{color:var(--mute);font-weight:600;font-size:11.5px;letter-spacing:.03em}
.heat td{text-align:center;padding:5px 0;border:1px solid var(--bg);width:5.2%}.heat th{border:0;white-space:nowrap}.heat thead th{text-align:center}.heat tbody th{font-weight:400;text-align:left;padding-right:10px}
.heat .mean th,.heat .mean td{font-weight:700;border-top:2px solid var(--ink)}.heat .na{color:var(--mute)}.heat .cl{font-style:italic}
figure{margin:18px 0 22px}figcaption{font:13px/1.45 ui-sans-serif,-apple-system,sans-serif;color:var(--mute);margin-top:6px}
.two{display:grid;grid-template-columns:1fr 1fr;gap:28px}@media(max-width:760px){.two{grid-template-columns:1fr}.heat{font-size:10.5px}}
svg{width:100%;height:auto;display:block;font-family:ui-sans-serif,-apple-system,'Helvetica Neue',sans-serif}
svg .grid{stroke:var(--rule);stroke-width:1}svg .tick{fill:var(--mute);font-size:11.5px}svg .lab{fill:var(--ink);font-size:13.5px}svg .num{fill:var(--ink);font-size:13px;font-weight:600}
svg .span{stroke:var(--rule);stroke-width:5;stroke-linecap:round}svg .bar0{fill:var(--rule)}svg .bar1{fill:var(--acc)}
.key span{display:inline-block;margin-right:16px}.key i{display:inline-block;width:10px;height:10px;border-radius:50%;margin-right:6px}
ol.next{padding-left:22px}ol.next li{margin-bottom:8px}.foot{color:var(--mute);font-size:14px}
"""

HTML = f"""<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Ranking signal from Jev on hard attributes</title><style>{CSS}</style>
<h1>What adds ranking signal to a typed judge, and what cannot</h1>
<p class="sub">Hosted Jev (TypeSafe <code>jev-latest</code>) on twelve hard attributes of three kinds of text, against a Fable reference. Measured 2026-09-19, 14:15–16:55 PT. Written for anyone who ranks things with model judgments; it assumes no knowledge of this project.</p>

<p class="lede">The question was how to get more ordering information out of each Jev call, in particular by eliciting richer pairwise ratios, and how far those ratios can be trusted to be consistent with one another.</p>
<div class="verdict">
<p><b>Richer readouts do not add information, and the consistency result says why.</b> Inside one call Jev’s pairwise ratios are transitive to within 1–3 % of their variance. Fifty-six ordered pairs over eight items are therefore eight numbers read fifty-six ways, and a nine-rung ratio ladder, a yes/no logit and a ten-point rating of the same window all read the same eight.</p>
<p><b>What Jev gets wrong it gets wrong consistently.</b> It agrees with itself across different windows at .79–.85 and with the reference at .59–.72. Repeating the question averages nothing away.</p>
<p><b>Signal comes from changing what is asked, not how the answer is read:</b> a definition beside the attribute name (+.08 to +.24), more items per state, and distinct concrete questions about each item, which reach .62–.67 at a fifteenth of the cost but only combine usefully when written for the genre of text.</p>
</div>

<h2>Setup</h2>
<p><b>The judge.</b> Jev takes a state (any text) and typed questions about it, and returns probabilities from a single prefill pass with no generation: <code>noul</code> gives P(yes) for a proposition, <code>score</code> gives a distribution over two to ten labelled levels. Questions in one call share the state and are answered independently. Price is $0.042 per million input tokens; this whole study cost $0.45 over 594 calls.</p>
<p><b>The texts.</b> Twenty-four each of arXiv abstracts, Manifund grant applications (2,500–6,000 characters) and recent public LessWrong comments (600–2,500 characters, usernames dropped), drawn with a fixed seed. The LessWrong cohort was run after the findings from the other two had been written down, so it is an out-of-sample check.</p>
<p><b>The attributes.</b> Twelve that are hard to operationalize and not one axis (first principal component 43–53 % of variance): self-containedness, high-status, low-status, poshness, technical calmness, earnestness, intellectual density, legibility to an outsider, coiled potential energy, craftedness, institutional insiderness, how much it rewards a careful re-read.</p>
<p><b>The reference.</b> Fable magnitude estimation on a ratio scale (median text = 1.0). Two independent replicas per cohort, each with its own item shuffle, neutral labels and attribute order; twelve reads, 712K tokens, run once. Replica-against-replica Spearman is the ceiling any judge can be measured against: {g("ceiling")} (arXiv / Manifund / LessWrong).</p>
<p><b>The design held fixed.</b> Random windows of eight items, three rounds, nine states per cohort, identical across every variant, so variants differ only in the questions. The score throughout is Spearman rank correlation between Jev’s fitted per-item latents and the reference, averaged over the twelve attributes.</p>

<h2>The result in one figure</h2>
<figure>{dots()}
<figcaption class="key"><span><i style="background:{COL['arxiv']}"></i>arXiv</span><span><i style="background:{COL['manifund']}"></i>Manifund</span><span><i style="background:{COL['lw']}"></i>LessWrong (no name-only run)</span> Mean rank agreement with the reference. Bars on the last row are the reference’s agreement with itself. The three propositions rows cost one call per window; the others cost twelve.</figcaption></figure>

<h2>The argument</h2>

<h3><span class="n">1</span>The readout is not the lever.</h3>
<p>With the definition in place, three ways of reading the same window tie: yes/no on “item x is stronger than item y” {g("noul")}, a nine-rung ratio ladder read as E[log ratio] {g("score9")}, a ten-point rating of each item {g("rate")}. Their combination gives {g("elab")}, no better than the best single one. An earlier pack on easier criteria (<code>jev-bits</code>) found the same from four other angles: every instrument and packing reached one plateau; regressing the reference on every field from one call beat the best single field by .00–.03; weighting each ratio by the inverse variance of its own distribution gained nothing at any budget; and the ratio ladder, while genuinely cardinal (r = .95 against true country ratios), compresses at .60–.67 nat per nat.</p>
<p class="so"><b>So:</b> the distribution over ratio rungs carries about 0.05–0.17 bits more than a yes/no on hard criteria and less on easy ones. That is the whole gain available from elicitation.</p>

<h3><span class="n">2</span>The reason: holonomy is nil, so a call holds eight numbers.</h3>
<p>Take the 56 ordered reads L(x,y) in one call. If Jev’s comparisons came from one latent per item, L(x,y) would equal u<sub>x</sub> − u<sub>y</sub> and the sum around any triangle would vanish. Measured on every call of every cohort: after removing the order effect of step 3, a per-item potential explains 97–99 % of the pairwise variance, and a triangle’s cycle sum has about a quarter of the spread of a single edge, where unrelated edges would give √3.</p>
<div class="two"><figure>{tri()}<figcaption>Triangle closure. Jev’s ratios compose: a→b→c→a returns almost exactly to where it started.</figcaption></figure>
<figure><table><thead><tr><th>cohort</th><th>readout</th><th>order</th><th>bias</th><th>curl</th><th>triangle</th></tr></thead><tbody>{holrows}</tbody></table>
<figcaption><b>curl</b>: share of pairwise variance no potential explains. <b>order</b>: sd of L(x,y)+L(y,x) over sd of L. <b>bias</b>: its mean, in sd units. <b>triangle</b>: as at left.</figcaption></figure></div>
<p class="so"><b>So:</b> the pair matrix has rank one in the additive sense. Asking more pairs, more rungs or more instruments of the same window re-reads eight numbers. Information per call is (items per state) × (what the judge resolves per item), and only the first factor is ours to raise by packing.</p>

<h3><span class="n">3</span>What is not a potential is mention order.</h3>
<p>The one large inconsistency is between L(x,y) and L(y,x). Their sum, which should be zero, has .45–.73 of an edge’s spread. The yes/no form also leans “yes” by +.35 to +.80 sd whichever item is named first; the ratio ladder leans slightly the other way (0 to −.29 sd) and is just as asymmetric pair by pair.</p>
<p class="so"><b>So:</b> asking each pair in both orders is the one within-call repetition that pays. It is the only place the pairwise matrix holds noise that averages out.</p>

<h3><span class="n">4</span>The remaining error is bias, not noise.</h3>
<div class="two"><div><p>Fit latents from a single round of three windows, where each item is seen in one context beside seven neighbours. A different round, with different neighbours, reproduces them at .79–.85. Against the reference the same single-round latents reach .59–.72, and pooling all three rounds moves that by only about .04.</p>
<p class="so"><b>So:</b> Jev has a stable opinion that differs from the reference. More rounds, more windows and more readouts converge on Jev’s opinion, not on the truth. The gap has to be closed by asking a different question.</p></div>
<figure>{selfc()}<figcaption>Ratio-ladder latents. The gap between the markers is systematic disagreement with the reference.</figcaption></figure></div>

<h3><span class="n">5</span>A definition is worth more than any instrument.</h3>
<p>Replacing the bare attribute name with a one-paragraph definition beside each question moved mean agreement from {f2(m["arxiv"]["bare"])} to {f2(m["arxiv"]["elab"])} on arXiv and {f2(m["manifund"]["bare"])} to {f2(m["manifund"]["elab"])} on Manifund, +.08 to +.24 by readout, for a quarter to a third more tokens. The largest single moves were Manifund self-containedness (.24 → .77) and arXiv technical calmness (.48 → .74). On LessWrong, run with definitions only, the same arm reached {f2(m["lw"]["elab"])}.</p>

<h3><span class="n">6</span>A bare name can silently invert a graded question.</h3>
<div class="two"><div><p>“How strong is item x on <i>low-status</i>” was read as status. Both graded readouts came back strongly negative on both cohorts while the yes/no proposition held. Nothing in the output signals this; only a reference reveals it. The definition repaired all three. The earlier pack saw the same failure with “concise” under a “how many times greater” wording.</p>
<p class="so"><b>So:</b> a graded field must spell out its direction. Where no definition is available, the yes/no proposition is the robust form.</p></div>
<figure><table class="heat"><thead><tr><th></th><th colspan="3">name only</th><th colspan="3">with definition</th></tr><tr><th>low-status</th><th>yes/no</th><th>ratio</th><th>rating</th><th>yes/no</th><th>ratio</th><th>rating</th></tr></thead><tbody>{lowbare}</tbody></table><figcaption>Rank agreement with the reference on “low-status”.</figcaption></figure></div>

<h3><span class="n">7</span>Distinct concrete questions are a second, cheap channel.</h3>
<p>Each attribute was decomposed by hand into five yes/no propositions about a single item (“Item 3 contains irony, sarcasm or jokes”, sign −1 for earnestness), signs fixed in advance, weights equal, nothing fitted to the reference. One call per window carries all 480 of them. Nine calls and half a cent reach {g("decomp")}, about a fifteenth of the tokens of the pairwise arm. Agreement climbs steadily with the number of propositions and has not flattened at five.</p>
<div class="two"><figure>{curve()}<figcaption>Mean agreement against number of propositions per attribute, averaged over all subsets of the five.</figcaption></figure>
<div><p>On the first two cohorts this channel carried different errors from the abstract question: adding it gave {f2(m["arxiv"]["both"])} and {f2(m["manifund"]["both"])}, +.03 and +.04, where three readouts of the abstract question had added nothing. It helped most where the abstract question was weakest: Manifund earnestness .32 → .50, arXiv poshness .23 → .50.</p>
<p><b>On the out-of-sample cohort it did not combine.</b> LessWrong gave {f2(m["lw"]["decomp"])} alone and {f2(m["lw"]["both"])} together against {f2(m["lw"]["elab"])} for the abstract arm. The propositions had been written with papers and proposals in mind; on comments, institutional insiderness fell from .88 to .61 and legibility from .84 to .57, while the ones whose wording carried over still helped (earnestness .66 → .73, intellectual density .83 → .89).</p>
<p class="so"><b>So:</b> decomposition is a real channel and the cheapest first pass measured, and its propositions are per-genre.</p></div></div>

<h3><span class="n">8</span>What did not work.</h3>
<p><b>Ten propositions instead of five</b> gave {g("decomp10")}: the second hand-written batch was weaker, and an equal-weight sum is hurt by one wrong-signed member (arXiv self-containedness .59 → .29). <b>Pruning without a reference</b>, by dropping propositions whose correlation with the rest is under .2, recovered arXiv to .67, left Manifund at .68 and hurt LessWrong (.66 → .61): one win, one tie, one loss, not yet a method. <b>Combining readouts</b> of one question added nothing anywhere.</p>

<h2>Every cell</h2>
<figure>{heat()}<figcaption>Rank agreement with the Fable reference per attribute. <b>name</b>: attribute name only. <b>defn</b>: definition beside the question (three readouts pooled). <b>props</b>: five concrete propositions. <b>both</b>: sum of the two. <b>Fable</b> (grey): the reference’s agreement with itself, the ceiling. Rust cells are inversions. arXiv institutional insiderness has no usable ceiling (abstracts barely differ on it) and should be read as unscored.</figcaption></figure>
<p>Still hard after every lever: LessWrong low-status (.41–.51), arXiv poshness (.22–.50) and craftedness (.34–.51), Manifund earnestness (.32–.62). The reference agrees with itself at .69–.94 on these, so the gap is the judge’s.</p>

<h2>What to build</h2>
<p>A sort on Jev is a window design. Pack as many items per state as the context allows, put every window of every round in flight at once, state the criterion as a definition with its direction explicit, ask pairs in both mention orders, and stop early: a second and third round together add about .04, and a fourth would add less. For a first pass or a large pool, replace the pairwise arm with a handful of concrete per-item propositions written for that kind of text. Treat the ratio ladder as cardinal only after a slope correction of about 1/.6.</p>
<h2>What to try next</h2>
<ol class="next"><li>Propositions drafted by Fable from the definition plus a few high and low reference items of the target genre, instead of by hand and cold. One read per cohort. This addresses the one lever that both worked and failed to transfer.</li>
<li>Proposition weights fitted on one cohort and tested on another. Nothing here was fitted.</li>
<li>A short analysis note per item placed in the state before the abstract question, to move the judge’s stable opinion rather than re-read it.</li>
<li>Larger windows. Step 2 says information per call scales with items per state; k = 8 was never varied here.</li></ol>

<h2>Replay</h2>
<p class="foot">Every Jev response is cached by request hash in <code>trace-&lt;cohort&gt;.jsonl.gz</code>; after <code>gunzip -k</code>, <code>ref.py</code>, <code>step.py</code>, <code>decomp.py</code>, <code>props_curve.py</code>, <code>prune.py</code>, <code>holonomy.py</code> and <code>report.py</code> run with no key and no spend. Item texts, the Fable prompts and raw magnitudes, and the propositions are in the same directory; this page is the record. Limits: n = 24 per cohort, one window seed, one reference model, propositions by one author.</p>
</html>"""
open(f"{HERE}/report.html", "w").write(HTML)
print("report.html", len(HTML), "bytes;", {c: {k: round(v, 3) for k, v in MEANS[c].items() if v is not None} for c, _ in COH})
