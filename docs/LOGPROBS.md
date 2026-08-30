# Logprobs Reference

This document is the reference for token logprobs in the judge gateway. All facts come
from live probes with dates and sample counts. The probe scripts are in
`research/notes/adaptive-logprobs-2026-07-19/`. When a provider changes, run the scripts again
and update the tables.

## Alternative counts for each model

The provider returns a list of alternative tokens for each output token position. The
`top_logprobs` parameter sets the requested list length. Each model serves a fixed
maximum count. The counts below come from the official OpenAI API (2026-07-18) and from
OpenRouter (2026-07-19, provider Azure, n=10 for each cell).

| Model | Count | Necessary condition |
|---|---|---|
| gpt-5.1, gpt-5.2 | 5 | `reasoning_effort` unset or `"none"` |
| gpt-5.4, gpt-5.4-mini, gpt-5.4-nano | 5 | `reasoning_effort` unset or `"none"` |
| gpt-5.5 | 5 | `reasoning_effort: "none"` |
| gpt-5.6-luna, gpt-5.6-sol, gpt-5.6-terra | 5 | `reasoning_effort: "none"` |
| gpt-4.1, gpt-4.1-mini, gpt-4o family | 20 | none |
| gpt-5, gpt-5-mini, gpt-5-chat-latest | 0 | no path exists |
| gpt-5.5-pro, o3, o3-mini, o4-mini | 0 | no path exists |

Each 5.x model that serves logprobs serves 5 alternatives, and not more. A request
for 6 or more gets HTTP 400 from OpenAI. Many OpenRouter hosts do not reject an
over-cap request. They return HTTP 200 with `logprobs: null`. Thus the request layer
must clamp `top_logprobs` to the known cap for each route — the seriate rail does,
via `seriate_logprob_route` (src/rerank/comparison/types.rs), which also pins
reasoning off where the 5.x unlock requires it. The 2026-04 census
(`diamond` archive) measured caps of 5 for Alibaba hosts and 20 for Cerebras.

## The reasoning gate

OpenAI blocks logprobs when reasoning is on. A request with `reasoning_effort` set to
`low`, `medium`, `high`, or `xhigh` and `logprobs: true` gets HTTP 400. This applies to
Chat Completions and to the Responses API. The only path to a 5.x PMF is
`reasoning_effort: "none"`.

Through OpenRouter, the equivalent unlock is `reasoning: {"effort": "none"}` or
`reasoning: {"enabled": false}`. Measured 2026-07-19: gpt-5.5 and gpt-5.6-sol served
logprobs in 10 of 10 calls with the unlock, and 0 of 1 without it. The gpt-5.4 family
served logprobs with and without the unlock (20 of 20 calls, provider Azure).

NOTE: Provider behavior through OpenRouter changes from day to day. The
`model_supports_logprobs` gate recorded 400 errors for gpt-5.4 on 2026-07-18. The same
route served logprobs on 2026-07-19. Capability data must carry a probe date.

## Structured outputs

Strict `json_schema` response format keeps logprobs at `effort: "none"`. The schema
also pins each field to a stable token position. This makes PMF extraction
deterministic. Loose `json_object` mode lets the model select its own keys. Do not use
`json_object` mode as an instrument.

## PMF quality cautions

CAUTION: A numeric `ratio` field spreads its probability mass across many magnitude
tokens. Measured on gpt-5.4-mini (2026-07-19, n=13): the top-5 alternatives of the
first ratio token held only 0.23 mean visible mass, and sampled answers ranged from 20
to 5000. Single-token answer alphabets (the letter ladder) do not have this problem.

CAUTION: A visible analysis field before the answer collapses the answer PMF. Measured
on gpt-5.6-sol (2026-07-19): the ratio token PMF went from 5 alternatives with 0.81
top-1 mass to a single token with 1.0 mass. The model commits during its own visible
text. Do not put free-text analysis before the answer tokens in a PMF instrument.

## Two-phase elicitation: reasoning context, then a logprob read

Productized 2026-08-30 as `ratio_letter_2p_v1` (src/rerank/comparison/seriate.rs;
default for the reasoning-native 5.5/5.6 families via `default_template_slug`).

There is no direct way to get a PMF from a reasoning pass. There is a two-phase way.
Phase 1 asks the model for an analysis at `reasoning_effort: "medium"` without a
verdict. Phase 2 sends a new request at `effort: "none"` with the analysis in the
context, and reads logprobs on the verdict tokens.

Measured on gpt-5.6-sol (2026-07-19, Chat Completions): the phase-2 PMF kept its
spread (0.81 and 0.19 on two ladder-adjacent tokens) and moved relative to the
one-shot PMF. The Responses API also accepts a `previous_response_id` continuation
from an `effort: medium` response into an `effort: "none"` request with logprobs.
But that path returned only 1 alternative per position (n=1). A fresh Responses call
returned 5. Use the Chat Completions two-phase shape until more probes explain this.

## Prompt cache and nonce perturbation

The OpenAI prompt cache makes distribution-stability measurement cheap. Put the stable
system prompt and entity text in a long prefix. Put the nonce at the end. Send
`prompt_cache_key` to help the cache route repeated prefixes to the same servers.

Measured on gpt-5.4-mini (2026-07-19, 1562 prompt tokens, 13 calls): 12 of 12 warm
calls hit the cache with 1280 cached tokens. Cached input tokens cost 10 percent of
the fresh price on this model family.

The harness's live rail reproduces this (measured 2026-08-07,
`probe_cache_openrouter.py`): OpenRouter routed openai/gpt-5.4-mini to Azure
and served 5 of 5 warm hits, with and without `prompt_cache_key` (3328 of
~3658 tokens cached; warm cost $0.00052 against cold $0.00277, an 81
percent discount). The key was not necessary for serial calls on this
route. Keep it for cache routing under concurrent load. CAUTION: a
`provider: {"only": ["openai"]}` pin routes through the account's OpenAI
BYOK integration, whose stored key is stale, and gets HTTP 401. Use the
unpinned default.

NOTE: Same-prompt repeats do not return identical logprobs. Three repeats of one nonce
gave top-1 ratio-token mass of 0.14, 0.12, and 0.26. Ten different nonces gave a mean
top-1 mass of 0.14 with a standard deviation of 0.06. Thus server noise and nonce
sensitivity had the same size in this measurement. A stability instrument must
average over repeats before it attributes variance to the nonce.

## The subscription backend (codex oauth)

The ChatGPT subscription backend (`chatgpt.com/backend-api/codex/responses`)
serves logprobs. The probe route is the cxp pool proxy, which adds the auth
headers. Probes are in `research/notes/codex-oauth-logprobs-2026-08-06/`, all measured
2026-08-06.

- The working shape is `include: ["message.output_text.logprobs"]` with
  `reasoning: {"effort": "none"}`. Measured: gpt-5.6-sol 10 of 10 calls,
  gpt-5.4-mini 10 of 10 calls.
- The backend rejects the `top_logprobs` parameter at each effort (11 of 11
  rejections). Each logprob entry has an empty alternatives list. Thus one
  call shows only the mass of the sampled token. To see the spread, sample
  the same prompt more times.
- The wire accepts `"none"` as an effort value for gpt-5.6-sol. The error
  text for `"minimal"` gives the supported set: none, low, medium, high,
  xhigh, max.
- The reasoning gate is the same as on the official API. A reasoning effort
  (or an unset effort) with the logprobs include gets HTTP 400.
- The two-phase shape from this document also operates on this backend:
  phase 1 at `medium`, then a phase-2 call at `"none"` with the analysis in
  the context and the logprobs include (n=1).

CAUTION: On this backend the `response.completed` event has an empty `output`
list. The output items with the logprobs come only in the
`response.output_item.done` events. A client that reads only the completed
event sees no logprobs.

### Cross-model two-phase (measured 2026-08-07)

Phase 1 can run on one model and phase 2 on another. Probe
`probe_crossmodel.py`, pair "1L ice vs 1L water" by mass (correct: water):

| Cell | Answer | Sampled-token mass |
|---|---|---|
| gpt-5.4-mini alone, effort none (n=5) | A (wrong) 5/5 | 0.43 mean, 0.15 sd |
| gpt-5.4-mini after a gpt-5.6-sol `medium` analysis (n=5) | B (correct) 5/5 | 1.00 |
| gpt-5.6-sol alone or after analysis (n=5 each) | B 5/5 | 1.00 |

The phase-1 instructions must not permit a verdict token. With the one-letter
system prompt left in place, the "analysis" degenerated to the verdict letter
(1 character) and phase 2 measured verdict copies, not reasoning transfer.
The corrected probe asserts a minimum analysis length.

The phase-2 read tracks the content of the analysis, not its presence
(measured 2026-08-07, pair "all ants vs all humans" by total mass, mini
baseline mass 0.66 with mixed tokens): a decisive analysis moves the read to
0.999, and a balanced analysis that resolves nothing keeps the spread (0.63,
mixed tokens). Thus the two-phase read is an evidence-tracking instrument.
A refutation pair must have measured baseline spread — a subjectively
"undecidable" pair (aurora vs eclipse, beauty) read 0.995 at baseline and
discriminated nothing.

### Prompt cache on this backend (measured 2026-08-07, small n)

The backend serves cached prefixes: a 2816-token stable prefix with a tail
nonce hit `cached_tokens: 2816` in 3 of 12 calls (1 of 6 without
`prompt_cache_key`, 2 of 6 with it; one account served every call). The
`prompt_cache_key` parameter is accepted, and the cxp pool proxy uses it as
the account-affinity key, so keyed traffic pins one account — necessary for
cache coherence because each account is its own cache namespace. Subscription
billing is $0 marginal, so cache here buys latency, not money; the economic
case for cache-aware resampling stays on the API side (see "Prompt cache and
nonce perturbation" above: 12 of 12 warm hits, cached input at 10 percent
price).

## Rerun commands

```
OPENROUTER_API_KEY=... python3 research/notes/adaptive-logprobs-2026-07-19/probe_openrouter_unlock.py
OPENAI_API_KEY=...     python3 research/notes/adaptive-logprobs-2026-07-19/probe_openai_direct.py
OPENAI_API_KEY=...     python3 research/notes/adaptive-logprobs-2026-07-19/probe_twophase.py
OPENAI_API_KEY=...     python3 research/notes/adaptive-logprobs-2026-07-19/probe_cache.py
python3 research/notes/codex-oauth-logprobs-2026-08-06/probe_codex_oauth.py
python3 research/notes/codex-oauth-logprobs-2026-08-06/probe_repeats.py gpt-5.6-sol
python3 research/notes/codex-oauth-logprobs-2026-08-06/probe_crossmodel.py
python3 research/notes/codex-oauth-logprobs-2026-08-06/probe_cache_codex.py --key
```
