# Two-pole scale-existence pilot — 2026-08-31

The afternoon-sized falsification check before any scale-existence statistics
get built: does the JCB battery separate an attribute where a scale certainly
exists from a paradigm taste attribute? Two new pools, same instrument, same
three cross-lab judges, one battery seed (s1), no cache. Total spend ≈ $6.4.

- **Oracle pole:** `code-correctness-two-pole-v1` — 12 merge_intervals
  implementations, truth = hidden-test pass fraction by direct execution.
- **Taste pole:** `funnier-two-pole-v1` — 12 one-liners, no truth by design.
- **Judges:** openai/gpt-5.6-terra, anthropic/claude-sonnet-4.6,
  google/gemini-3.1-pro-preview.

## Headline table (202 comparisons per cell)

| judge | pole | coherence | harmonic | signal (nats) | flip | polarity ρ | magcal | refusals |
|---|---|---|---|---|---|---|---|---|
| terra | code | .873 | .834 | 1.64 | .00 | −.98 | .816 | 0 |
| sonnet | code | .888 | .600 | 1.41 | .00 | −.98 | .600 | 0 |
| gemini | code | .947 | .934 | 1.80 | .00 | −.95 | .979 | 1 |
| terra | funnier | .868 | .841 | 0.54 | .10 | −.62 | — | 0 |
| sonnet | funnier | .794 | .621 | 0.49 | .35 | −.76 | — | 0 |
| gemini | funnier | .883 | .788 | 0.74 | .05 | −.88 | — | 4 |

## Cross-judge latent agreement (Spearman, n=8 items)

- code: .929 / .952 / .929 (mean **.94**)
- funnier: .738 / .714 / .690 (mean **.71**)
- code vs hidden-test truth: terra .690, sonnet .714, gemini **.857**

## Verdict

**The instrument separates the poles.** Code lands high/high (high coherence,
high cross-judge agreement, 3× the signal, zero order flips, near-perfect
polarity inversion, real truth recovery). Funnier keeps per-judge coherence
high while signal collapses 3×, order flips and refusals appear, polarity
inversion degrades, and cross-judge agreement drops to .71 — the
consistent-committee-member signature, mid rather than collapsed (three
frontier judges share a lot of comedic culture).

**The judge-stable-curl separator did NOT validate at this n.** Per-triangle
curl locations (16 triangles on the core graph) correlate only weakly across
judges on the taste pole (r = +.16/+.34/+.28) and are not clean zero on the
oracle pole (+.15/−.11/+.41). At 8 items the curl-location statistic has no
power; validating the separator needs bigger pools and/or repeat draws, not
this battery scale. High-coherence/low-agreement cells therefore still read
"plural OR idiosyncratic".

Replay: `cardinal bench --models <slugs> --pool research/data/pools/<pool>.json
--battery-seed 1 --no-cache` (specs in this pack; `spec-code-s1.json` is the
deepseek-v4-flash smoke run's spec, identical to the code spec).
