# Public benchmark naming — candidate record (2026-08-20)

Brainstorm + collision/domain checks for the public name of the ratio-
consistency benchmark (JCB stays the versioned technical slug in docs and
packs regardless; the locked 2026-08-12 naming map in
`north-star-ontology-2026-08-11.md` is unchanged by this note).
**Status: OPEN — operator decision.** Recommendation: Holonomy.

## Constraints the name must satisfy

1. Model-card citable: "scores 0.71 on ___" — short, noun-shaped.
2. Failure-card captionable: benchmarks spread as artifacts of failure.
3. Register: common-noun-friendly, no gotcha branding, dignified enough
   for a lab to print voluntarily.
4. No collision with the occupied judge-eval namespace.

## Landscape (checked 2026-08-19/20)

- **Taken**: JudgeBench (arXiv:2410.12784), JudgeSense (arXiv:2604.23478
  — uncomfortably adjacent: prompt-sensitivity of LLM judges), "Rating
  Roulette" (arXiv:2510.27106, self-inconsistency), "Rigorous Bench"
  (deep-research agents — dilutes RIGOR slightly).
- **Unclaimed in eval space**: holonomy, voir dire (for AI judges),
  cross-exam.
- **Domains** (whois): holonomy.org / holonomy.io / voirdire.ai squatted;
  holonomybench.org, rigorbench.org, ratiobench.org, crossexam.org,
  judgecoherence.org free. Per the locked map the site is
  pairwiseratio.org; the benchmark needs no own domain.

## The spread

### A. Mechanism names (math-true, lab-credible)

- **Holonomy** ★ recommended. Every probe is a loop in presentation
  space (swap-and-return, negate-twice, paraphrase-and-back, compose
  around a cycle); a belief is judgment with zero holonomy. Not a
  metaphor — the actual mathematics of the battery (Hodge machinery
  already speaks it). Unclaimed, one word, dignified; press gloss is one
  line ("its opinions don't add up around a loop"). Nit: holonomy the
  quantity is low-good while the headline JUDGE SCORE is high-good —
  cosmetic, the headline keeps its own label.
- **Kirchhoff** — "judgments must obey Kirchhoff's law": delicious, but
  names only the cycle axis and borrows a person.
- **Fixpoint** — thesis-literal ("a belief is a fixed point of the
  transformations that shouldn't matter"); collides with PL jargon.
- **Curl**, **Gauge** — overloaded (Gauge has the instrument pun but is
  startup-soup).

### B. Property names (model-card-native)

- **RIGOR** — runner-up. Honest backronym: *Ratio Invariance under
  Group Orbits and Reciprocity* (the orbit transform is literally a Z₂³
  group orbit). Both polarities work: "RIGOR 0.83" in a card, "lacks
  RIGOR" on a failure card. Cons: Rigorous-Bench adjacency; arXiv
  backronym trope reads slightly off-register.
- **SANE / SOUND** — catchy backronyms; gimmick-decay fast; "insane
  model" framing is a partnership liability.

### C. Ritual names (press-native)

- **Voir Dire** — conceptually perfect (the qualification exam a judge
  must pass before serving — BENCHMARK.md's own framing) and pre-names
  the certification product ("passed voir dire"). French, two words,
  bad in a metrics table. **Use as copy: "the voir dire for AI
  judges".**
- **Cross-Exam** — same question asked different ways to catch the lie;
  vivid, slightly aggressive.
- **Perjury** — press catnip, lab poison. Failure-card vocabulary only.

### D. Verdict names — Survives / Holds / Steady: better as copy; the
site headline "Does the judgment survive?" already owns this register.

## Surface test-fits

- Leaderboard row: `RIGOR: 0.58` frictionless; `Holonomy: 0.58` fine
  (see polarity nit above).
- Failure card: "Holonomy caught gpt-5.4-mini: A is 3× B, B is 2× C,
  C is 1.5× A — transported around the loop, the belief doesn't
  return." vs "gpt-5.4-mini lacks RIGOR" (brutal; insult-first).
- Press subhead: "Holonomy — the voir dire for AI judges" composes;
  RIGOR + voir dire double-brands.

## Decision shape

Pick one: **Holonomy** (elegance, truth, distinctiveness) or **RIGOR**
(model-card pragmatism). Either way: voir dire/cross-examination stay as
copy; JCB stays the technical slug; propagate the chosen name to
`research/notes/PUBLIC_BENCH.md` §Naming and the pairwiseratio.org copy when
decided.
