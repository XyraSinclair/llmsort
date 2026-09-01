# Scale-existence battery: 6 attributes × 4 judges (2026-08-31)

Does a shared one-dimensional scale exist for an attribute, or is quality
plural? Six pools spanning oracle-anchored (arithmetic accuracy, summary
faithfulness, code correctness), contested (argument cogency, employee
performance), and taste (prose beauty) tiers; four judges
(openai/gpt-5.6-terra, google/gemini-3.1-pro-preview,
anthropic/claude-haiku-4.5, qwen/qwen3.7-max); battery seed 1, no cache;
202 calls per judge-pool; total $19.18.

Instrument note: Alibaba's provider caps `top_logprobs` at 5 and enables
thinking by default; the qwen judge ran with
`CARDINAL_PAIRWISE_LOGPROBS_TOP_N=5 OPENROUTER_DISABLE_REASONING=1`
(battery specs asserted byte-identical to the other judges' run). Its
confidence channel therefore reads a 5-deep PMF where the others read 20.

## Headline table (means over 4 judges)

| pool | coherence | harmonic coh | cross-judge ρ | truth ρ | signal | refusals |
|---|---|---|---|---|---|---|
| arithmetic-accuracy | .895 | .865 | .937 | .958 | 2.29 | 14 |
| argument-cogency | .919 | .733 | .845 | .899 | 1.13 | 7 |
| summary-faithfulness | .898 | .700 | .972 | .982 | 1.60 | 4 |
| code-correctness | .912 | .694 | .758 | .618 | 2.14 | 9 |
| prose-beauty | .927 | .857 | .897 | — | 1.00 | 7 |
| employee-performance | .919 | .711 | .984 | — | 0.85 | 9 |

## Findings

1. **Scale-existence is the norm at spine granularity.** No pool lands in
   the high-coherence/low-agreement cell. Even the paradigm taste
   attribute (prose beauty, ρ .897) and the flagship contested attribute
   (employee performance, ρ .984 — the battery's HIGHEST) behave
   scale-like when items span real quality variation. Two registered
   predictions (beauty high/low, employee mid/low-mid) missed in the
   same direction: we over-predicted plurality.
2. **The criteria-trading signature is within-judge, not cross-judge.**
   Contested/trading pools depress harmonic coherence (.69–.73 vs .86 on
   arithmetic/beauty): judges disagree with THEMSELVES on
   closely-matched, criteria-trading items more than they disagree with
   each other on rankings.
3. **Cycle-energy location is judge-idiosyncratic.** Cross-judge Spearman
   of harmonic energy fraction across pools spans −0.60…+0.49 (mean
   ≈ −0.08). The hypothesis that frustration concentrates on the same
   cycles for all judges (a shared plural structure) fails its second
   test; "high coherence + harmonic energy" reads as idiosyncratic
   instability, not shared plurality.
4. **Magnitude calibration tracks oracle auditability.** Faithfulness
   (14 auditable facts) shows magcal 3.5–4.6; arithmetic 1.3–1.6;
   cogency 1.1–1.4; code 0.3–1.0. Judges' ratio *magnitudes* carry real
   information exactly where an external error count exists.
5. **Truth recovery splits by judge capability on code.** merge_intervals
   bug-ranking: terra .843, gemini .916 vs haiku .434, qwen .277 — the
   truth pole is capability-gated, unlike arithmetic/faithfulness where
   all four judges recover truth ≥ .90.

## Files

- `spec-<pool>-s1.json` — frozen battery specs (item texts, pair
  schedules, truths where defined)
- `reports-<pool>.jsonl` — 4 judge reports each (verbatim instrument
  output incl. per-call log-ratios)
- `analysis.json` — the table above plus per-pair agreement matrices
