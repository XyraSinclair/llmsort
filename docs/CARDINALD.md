# cardinald — the judgement-run daemon

`cardinald` (`src/bin/cardinald.rs`) is an HTTP daemon for portable
single-axis judgement runs. It accepts a finite candidate set, runs the
`cardinal.judgement-run.v1` flow, and persists each run to disk. If you
configure ClickHouse landing, it also lands completed runs there. This
document is the contract for the five endpoints. The source is canonical
when the two disagree.

## Start the daemon

```bash
cargo run --bin cardinald
```

Configuration comes from environment variables:

| Variable | Default | Meaning |
|---|---|---|
| `CARDINALD_ADDR` | `127.0.0.1:8093` | Listen address |
| `CARDINALD_MAX_CONCURRENT_RUNS` | 4 | Runs that execute at the same time |
| `CARDINALD_MAX_QUEUED_RUNS` | 32 | Admission cap for queued runs and schedules |
| `CARDINALD_RUN_DIR` | `.cardinald/runs` | Run metadata and terminal records |
| `CARDINALD_OPENROUTER_KEY` | unset | Fallback provider key for adaptive runs |
| `CARDINALD_CLICKHOUSE_URL` | unset | ClickHouse landing target |

NOTE: When `CARDINALD_CLICKHOUSE_URL` is not set, completed runs stay in
the pending-landing files and do not land. The daemon reports this only on
stderr.

At startup the daemon replays pending landings and recovers interrupted
runs. A run that was `running` at shutdown becomes `failed` with the
message "daemon restarted before the run finished; resubmit".

## Request limits

All request bodies obey these caps. The daemon rejects a request above a
cap with `400`.

- 2 to 200 entities for each run
- 8192 bytes maximum for each entity text
- 4096 bytes maximum for `axis_prompt`
- The comparison budget is fixed: 8 × entity count (counterbalanced pairs)

## Endpoints

### `GET /healthz`

Returns `ok`.

### `POST /v1/estimate`

Returns a worst-case spend bound for a run request, without execution.

Request body: `entities` (list of `{id, text}`), `axis_key`,
`axis_prompt`, `requested_k`, `model`.

Response: `max_spend_nanodollars`, `planned_comparisons`, `price`
(`model`, `prompt_nanodollars_per_token`,
`completion_nanodollars_per_token`, `as_of`), and `bound_method` (the
formula as text). The bound is ceil(1.25 × comparisons × price of the two
longest texts). Unknown models get `409 price_unknown`.

### `POST /v1/runs` — adaptive mode

Starts a judgement run that calls the provider through OpenRouter.

Request body: the estimate fields plus `privacy` (`public` or
`private`), optional `owner_scope`, optional `lens`. Public runs must
have an empty `owner_scope`. Private runs must have a nonblank
`owner_scope`. The provider key comes from the `x-provider-key` header,
with `CARDINALD_OPENROUTER_KEY` as the fallback. Provider errors are
scrubbed so the key cannot leak into responses.

Response: `202` with `{run_ref, status: "running"}`. A full queue gives
`429`.

### `POST /v1/schedule` — external lane, step 1

Returns a stateless comparison plan for an external harness. The response
contains `schedule_version`, `template_slug` (`canonical_v2`),
`template_hash`, `seed`, `schedule_digest`, and `comparisons` — each with
`comparison_index`, `entity_a_id`, `entity_b_id`, `swapped`,
`system_prompt`, and `user_prompt`. Prompts come from the same
`canonical_v2` renderer as the adaptive path, at the same 8×N budget,
with both presentation orders for each pair.

The `schedule_digest` is a SHA-256 over the template hash, seed, axis,
budget, and every entity id and text. It binds later results to this
exact rendering. Schedule calls ride the same admission gate as runs.

### `POST /v1/runs` — external mode, step 2

Submits the answered comparisons in one shot. Zero provider calls occur.

Request body: the adaptive-mode fields plus `mode: "external"` and
`external`: `{harness, harness_version, model, seed, schedule_digest,
results}`. Each result carries `comparison_index`, `entity_a_id`,
`entity_b_id`, `swapped`, `higher_ranked` (`A` or `B`), `ratio`,
`confidence`, optional `refused`, and optional token counts.

Validation is strict, and each failure gives `400`:

- `harness` is disclosed free data, not an allowlist (openpriors
  invariant 4, executed 2026-09-06): 1–64 printable characters, trimmed,
  with the platform-hosted names (`llmsorting`, `cardinal-harness`,
  `ratiometer`) reserved — an external run claiming one would masquerade
  as platform-attested
- `schedule_digest` must match the digest for this request and seed
- Every scheduled comparison must have an answer. Refusals count as
  answers.
- Every entity needs at least one non-refused measurement.

Accepted results become well-formed `ComparisonTrace` rows carrying the
declared harness as provenance at zero cost, and feed the same fitter
and store as adaptive runs.

### `GET /v1/runs/{run_ref}`

Returns run state for both modes. Fields: `run_ref`, `status`
(`running`, `completed`, `cancelled`, `failed`), `privacy`,
`owner_scope`, `lens`, `axis_key`, `axis_prompt`, `model`, `entity_ids`,
`entity_text_hashes` (SHA-256 hex of each entity text, aligned with
`entity_ids`), and `created_at`. Terminal completed runs add `response`:
`scores` (`entity_id`, `rank`, `latent_mean`, `latent_std`, `z_score`,
`percentile`), `stop_reason`, `comparisons_used`, provider token counts,
`cost_nanodollars`, and `cost_is_estimate`. Failed runs add `error`.

An unknown or malformed `run_ref` gives `404`. Valid references have the
shape `jrun_<32 hex>` (UUIDv4).
