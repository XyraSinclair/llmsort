# E9 — head-to-head: ratio_letter_v1 (single-token PMF) vs canonical_v2 (JSON) (2026-08-19)

First gate of the NORTH migration (research/notes/NORTH.md): measure the decree's
premise — is the logprob-native single-token rail already at least as
good, per dollar, as the JSON rail it would replace?

**Setup:** `llmsort sort`, 6 bench-corpus items (`corpus.txt`),
"depth of insight about living well", openai/gpt-4.1-mini (20 logprob
alts), 24 comparisons per arm (full counterbalanced ring budget), fresh
pack-local caches (`*-cache.sqlite`, committed; replay each arm with
`--cache <pack sqlite> --cache-only` → $0).

**Measured (this pack's replayable runs):**

| | ratio_letter_v1 | canonical_v2 |
|---|---|---|
| cost (24 comparisons) | $0.0048 | $0.0044 |
| stat error (posterior mean) | **±0.020** | ±0.464 |
| order residual | **0.034 nats/pair** | 0.178 nats/pair |
| rank risk (top-k flip) | **1.879** | 2.787 |
| frustration (cyclic energy) | 16.9% | 11.5% |
| evidence mode | 24/24 logprob PMF, visible 1.00 | parsed JSON |
| ranking agreement | ρ = 0.886 (two adjacent swaps in the top 4) | — |

**Reading:** at IDENTICAL cost today (input tokens dominate; no cache
warm-up in this design), the PMF rail delivers ~23× tighter statistical
error and ~5× lower order asymmetry per call — one token position's
logprobs carry the whole posterior, where the JSON rail yields a single
point estimate. The rankings agree at ρ 0.886. This is the evidence-per-
dollar inversion NORTH predicts, before any prompt-cache exploitation
(the family sweep, E10, is where the cache discount compounds it).

**Honest flags:**
- Frustration is HIGHER on the letter rail (16.9% vs 11.5%; first run
  pair read 19.8% vs 8.2%). More sensitive instrument or more cyclic
  judge? E10 must separate these (the reliability axes will say).
- n=6 items, one attribute, one model. Cross-run repeat of the same arm
  moved order flips 3/12→2/12 and frustration 19.8→16.9 — single-run
  numbers carry that much slop; the direction of the stat-error gap
  (23×) dwarfs it.
- Cost parity here is NOT the endgame claim; the cache-discounted
  attribute family (marginal variant ≈ tail tokens) is, and it is
  unmeasured until E10.

**Next:** E10 family sweep (`cardinal family`): pair-prefix cached,
{A, A′, ¬A} × both orders; report cached-token fraction, obs/$, and the
reliability reading computed from the same calls.
