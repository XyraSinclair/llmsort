# jev-bits: how much ordering information one Jev call carries (2026-09-19)

Judge: TypeSafe `jev-latest`, `POST /v1/systemone`. 4,660 calls, 52.8M input tokens, $2.22, p50 326 ms,
p95 886 ms, 12 workers, no 429s, no retries. Executed 12:21–12:35 PT. Replay: `gunzip -k trace-*.jsonl.gz`,
then `python3 run.py <cohort> [arms]` reads the trace and needs no key; `analyze.py <cohort> [--curves]`,
`stack.py <cohort> <arm>`, `weight.py`.

Question: llmsort reads a ratio PMF from chat models by logprob side channel. Jev returns a PMF as its
contract, takes many isolated questions per call, and bills only input. Which question type ("field") and
which state packing give the most ordering information per call and per token?

## Design

Two axes, crossed on identical items.

Packing (what one state holds): **P** one pair, both state orders (1,560 calls at n=40); **W** windows of
8, 20 rounds of random partitions (100 calls); **F** the whole cohort in one state, 4 shuffled labelings,
questions chunked at ~45K tokens; **S** one item, absolute 10-level rubric (40 calls).

Instrument: **noul** "item x is greater than item y" (logit); **score5** and **score9** ratio ladders
(rungs 10/3/1 and 8/4/2/1.3/1, read as E[log ratio]); **thermo** eight nouls "x is at least r times y"
(a CDF built from questions); **choice** top-1 and bottom-1 over the state's items; **rate** 10-level
standing of one item among those shown. P carries every pair instrument in the same call, so instruments
are compared on identical states. noul and score9 are asked in both mention orders everywhere.
Wording arms on W's exact windows: **T** terse (criteria stated once in the state, short rungs), **U**
terse score9 with polarity-safe wording ("how strong is item a relative to item b", rungs "far weaker:
1/8 or less" … "far stronger: 8 times or more").

Cohorts and reference: `arxiv` and `hn_comments` from `research/batteries/multi-criteria-bench` (40 items,
3 criteria each; reference = mean of gemma-4-31b's separate and joint arm z-scores, so rho is agreement
with another judge whose own split-half is .73–.94, not truth); `countries` (population) and `rivers`
(length), 16 items each, reference = log of the true value.

Fit: unweighted least squares on directed observations; item instruments by within-call-centred means.
`bits` = 0.5·log2(1/(1−r²)) for ONE directed observation against the reference difference (Gaussian-channel
reading; observations sharing a state are not independent, so bits do not add across questions — see 3).

## API facts (probe.py)

- Questions are isolated: one score question alone vs beside 60 others moved ≤ 0.01, the run-to-run floor.
- Probabilities come quantized to 0.01. Repeats differ by 0.01–0.02.
- 1,000 questions in one call: 1.1 s. 40 abstracts in one state (13K tokens): 284 ms.
- Questions bill as input (~13 tokens + their text; a verbose 9-rung ladder is ~200 tokens, a noul ~60).
  In W and F the questions, not the items, are most of the bill.
- Score is capped at 10 levels (HTTP 400 at 11).

## Findings

1. **Every instrument and packing reaches the same plateau; the plateau is the judge, not the readout.**
   Mean rho at full data, arxiv / hn_comments: P noul .78/.87, P score9 .80/.80, W noul .81/.88,
   W score9 .83/(.40, see 2), W rate .82/.85, W choice .83/.83, F noul .82/.86, S rate .75/.74.
   Per criterion Jev sits at gemma's own reliability on five of six (evidence .92, civil .93, novelty .88,
   informative .85–.90, concise .82–.89 by noul) and low on arxiv clarity (.55–.72), gemma's known soft cell.

2. **The ratio wording is polarity-fragile.** On hn_comments `concise`, "how many times greater is item x
   than item y" read as length: score9 rho −.61 (W), −.63 (F), −.73 (T), score5 +.36 and score9 +.63 in P,
   while the noul held at +.82 to +.89. Rewording to "how strong is item a relative to item b" (arm U)
   gives +.87 on `concise` and .897 mean over the cohort, the best of any arm there. "Greater" invites a
   surface magnitude; a ratio field must name strength on the criterion.

3. **More fields on the same state add almost nothing.** Regressing the reference difference on every
   pair instrument from one P call (5-fold CV) against the best single instrument: arxiv novelty .93 vs
   .91, clarity .51 vs .48, evidence 1.37 vs 1.32; hn informative .86 vs .84, civil 1.06 vs 1.03, concise
   .72 vs .61; rivers .32 vs .27. Errors on one state are shared. The one in-call addition worth having is
   the mirrored mention order (noul +.03 to +.10 bits; it cancels a yes-bias of up to +.6 sd).

4. **The PMF helps where the judgment is hard and costs where it is easy.** One observation, score9 vs
   noul, bits: arxiv clarity .51 vs .34 (W), rivers .66 vs .49 (W), informative .91 vs .79 (W), civil .97 vs
   .90 (W), novelty .89 vs .84 (W); on the easiest criterion it loses, evidence 1.18 vs 1.25 (W), 1.15 vs
   1.32 (P). score9's mention-first bias is usually smaller (P informative +.22 → +.02 sd, P clarity +.58 →
   +.18, W clarity +.48 → +.36). The thermometer costs 2.5× score9's tokens and beat it only on countries
   (1.70 vs 1.64) and in the polarity case of finding 2. score5 ≈ score9.

5. **Packing is where the information per call is.** Context helps each judgment as well as amortizing
   the state: rivers bits per observation .14–.26 in P, .49–.66 in W, .47–.61 in F; arxiv clarity .38 (P)
   → .51 (W) → .53 (F). A W call returns 28 pairs × 2 orders per criterion for one state.

6. **Cost to the plateau, n=40, mean over 3 criteria** (learning curves, 12 resamples; tokens are the
   instrument's own questions plus the state):

   | arm · instrument | calls | K tokens | rho arxiv | rho hn_comments |
   |---|---|---|---|---|
   | W choice (top+bottom) | 20 | 31–68 | .79 | .81 |
   | W rate | 20 | 49–87 | .80 | .83 |
   | T noul (terse) | 20 | 80 | — | .88 |
   | W noul | 20 | 114–162 | .80 | .86 |
   | P noul | 156 | 69–135 | .78 | .85 |
   | W score9 / U score9 | 20 | 197–302 | .84 | .88 (U) |
   | S rate (absolute) | 40 | 26 | .75 | .74 |

   Twenty window calls run concurrently are one round trip: under a second of wall time and under a
   cent for a 40-item, 3-criterion sort at the judge's ceiling. Pairs need 8× the calls for the same rho.
   Absolute pointwise rating is cheapest and lands .06–.13 lower.

7. **Terse wording is not free.** Moving the criterion text into the state saved 30–45 % of tokens and
   held or gained on hn_comments (noul .875 → .893) but lost on arxiv (noul .81 → .74, score9 .83 → .79,
   clarity .72 → .61). A soft criterion wants its definition beside the question.

8. **Truth cohorts.** countries: every instrument .95–.99, one observation 1.3 (noul) to 1.7 bits (score5,
   thermo); the graded read is worth +.3 bits against a true cardinal. rivers (true ratios mostly < 1.3):
   .44–.76, a knowledge ceiling; F choice and rate did best (.76, .74).

9. **The PMF's spread is not usable precision** (`weight.py`). Inverse-variance weights from each score9
   PMF against the unweighted fit, 24 resamples: arxiv W .620/.769/.817 vs .624/.779/.821 at 5/10/20 calls;
   hn_comments U .678/.809/.872 vs .682/.806/.869. No gain at any budget.

10. **The ratio read is cardinal but compressive.** countries, E[log ratio] against the true log ratio:
    slope .60–.67 nat per nat at r .95 in P, W and F alike, with observations pinned at the ladder's end
    rung (2.08 nat for 8×) while true gaps run past 4 nat. Latents from this ladder understate large ratios
    by about 40 %; an AHP use needs a slope correction or wider rungs.

## What this says about the instrument to build

The per-pair channel is 0.3–1.4 bits and fixed by the judge; no readout on the same state widens it.
Information per call scales with distinct states and with items per state. The sort for Jev is therefore
a window design, not a pairwise one: random k=8 partitions, all windows of all rounds in flight at once,
per window the item-level fields (choice top/bottom, rate) for the cheap pass and score9 in both mention
orders, polarity-safe wording, where the criterion is soft or the output must be cardinal. Roughly four
rounds (n/2 calls) reach the plateau at n=40.

## Not yet measured

- n=40 only, one seed for windows and labelings, two judged cohorts; rho against gemma is agreement.
  lw and hn_top have no committed gemma summary here and were not run.
- Fitting rate + choice + score9 jointly is untested.
- Window size between 8 and 40, and how rounds-to-plateau grows with n, are untested; F's curve is
  confounded by chunking (a chunk is a contiguous block of one labeling's pairs).
- Token costs per instrument are estimated (chars/4 + 12 per question, scaled to each call's billed total).
