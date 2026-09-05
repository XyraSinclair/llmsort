# freelane

Free-token elicitation driver for cardinald. It continuously re-elicits the
public ledger's existing (lens, axis) cells across OpenRouter's free-model
pool (`:free` slugs priced 0/0), up to `FREELANE_CONCURRENT_RUNS` runs at a
time — one per distinct model — paced to free-tier limits. OpenRouter's free
limits are account-wide (20 req/min, 1000 req/day; verified 2026-09-04), so
concurrency does not multiply throughput on one account — the daily budget
binds. It buys even accrual across many judges at once, immunity to one slow
model stalling the lane, and the right shape for when more quota (BYOK,
other services) arrives.

What DOES multiply throughput is more lanes: `FREELANE_PROVIDERS` adds
independent free-tier providers (Cerebras, Gemini, …) as JSON — each an
OpenAI-compatible endpoint with its own key, rpm, per-model daily budgets,
and concurrency. freelane routes their runs through the same cardinald via
the per-run `provider_base_url` field and a loopback `x-provider-key`
header; every lane's rows land in the same private ledger under the same
owner scope, keyed by the provider's bare model slug (which never collides
with OpenRouter's namespaced `vendor/model:free` shape — freelane refuses
to start on a duplicate).

```
FREELANE_PROVIDERS='[{"name":"cerebras","base_url":"https://api.cerebras.ai/v1",
  "key_env":"CEREBRAS_API_KEY","rpm":10,"concurrent_runs":2,
  "models":[{"slug":"gpt-oss-120b","daily":350},
            {"slug":"qwen-3.8-27b","daily":350},
            {"slug":"gemma-4-31b","daily":300}]}]'
``` The product is model-diverse priors: the same axes, the same
entities, judged by every free model — landed with full provenance and
re-fittable forever.

## The private-only invariant

Every freelane run is submitted with `privacy: "private"` and lands in
`scry_judgements_private`. This is load-bearing, not caution: the public
`scores_current` projection is a ReplacingMergeTree keyed
`(lens, axis_key, entity_id, entity_hash)` — no model in the key — so a
public free-model run would displace the curated scores behind the public
boards. Free judges earn public standing through the Judge Coherence
Benchmark and rank-agreement analysis, as an explicit editorial decision;
freelane never makes that decision itself.

## State model

There is no state file. A (lens, axis, model) cell is done iff
`scry_judgements_private.comparisons` holds rows for it under
`FREELANE_OWNER_SCOPE`. The driver re-derives its work list from the ledger
on every sweep, so restarts and crashes are always safe; at most the
in-flight run is left to cardinald, which persists and lands it on its own.

## Pacing

Two mechanisms, both continuous (never calendar windows):

- Per-request: each run is submitted with `comparison_concurrency: 1` and
  `min_request_interval_ms = FREELANE_CONCURRENT_RUNS × 60000 / FREELANE_RPM`
  (capped at cardinald's 60s maximum), so the combined paced floor across all
  concurrent runs stays at `FREELANE_RPM`; cardinald's
  `PacedGateway` enforces the floor between provider calls, and paced runs
  get ZERO gateway retries — retries fire below the pacer, so they multiply
  the real request rate and turn one seed 429 into a self-starving storm
  that consumes the shared free window with doomed re-attempts (observed
  live 2026-09-04). With retries off, the paced rate is the real rate and a
  failed call honestly consumes engine budget.
- Per-day: a leaky bucket with capacity `FREELANE_DAILY_BUDGET` refilled at
  that budget per 86400s; a run is charged its 8·n attempt budget before
  submission. At boot the bucket is seeded from the ledger (capacity minus
  comparisons landed in the trailing day), so restarts cannot mint fresh
  budget.

The driver polls a submitted run to its terminal state, budgeting 90s per
comparison (free-tier latency runs ~30s/response and stacks serially at
concurrency 1); the deadline exists only to catch a wedged daemon, and an
abandoned run is still cardinald's to finish and land — the next sweep
sees the landed cell as done.

Models that fail a run cool down (1h doubling per consecutive failure,
capped 6h) and the sweep moves to the next model. Free-model saturation
(upstream 429) and account-settings exclusions (training-policy 404s)
both surface as failed runs and are absorbed by the cool-down.

## Config (environment only)

| Variable | Default | Meaning |
| --- | --- | --- |
| `FREELANE_CLICKHOUSE_URL` | required | ClickHouse HTTP endpoint (userinfo in URL becomes basic auth) |
| `FREELANE_CARDINALD_URL` | `http://127.0.0.1:8093` | cardinald loopback |
| `FREELANE_RPM` | 10 | request-per-minute floor spacing (clamped 1–60; the account-wide free window is ~20/min — leave headroom) |
| `FREELANE_DAILY_BUDGET` | 900 | elicitation requests per rolling day |
| `FREELANE_CONCURRENT_RUNS` | 4 | max in-flight OpenRouter runs, one per distinct model (clamped 1–12) |
| `FREELANE_PROVIDERS` | empty | JSON array of extra provider lanes (see above); each model gets its own daily bucket |
| `FREELANE_OWNER_SCOPE` | `freelane` | owner scope on landed private rows |
| `FREELANE_MODEL_DENYLIST` | empty | comma-separated slugs to skip |
| `FREELANE_MAX_ENTITIES` | 60 | per-axis entity cap (top by current score) |

The OpenRouter key is cardinald's problem (`CARDINALD_OPENROUTER_KEY`
fallback). Extra-lane keys are read by freelane from each lane's `key_env`
and travel only over the 127.0.0.1 cardinald socket as `x-provider-key`;
they are never logged or landed.

`freelane --plan` prints the discovered axes, free pool, pending cells and
request estimate, then exits without submitting anything.

## Running as a systemd user unit

Template at `ops/freelane.service`. Install for the operating user (never a
system unit):

```
install -D ops/freelane.service ~/.config/systemd/user/freelane.service
systemctl --user daemon-reload
systemctl --user enable --now freelane
journalctl --user -u freelane -f
```
