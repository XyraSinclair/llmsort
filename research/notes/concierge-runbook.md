# Concierge runbook — grand-plan Move 2 (2026-08-29)

The lead runs a stranger's real list and sends back a legible packet.
Everything below is existing CLI output; nothing here is a feature.
Guardian consensus (two→four escalation, 2026-08-29): this half-page and
one dry run are the only pre-contact build allowed; all engineering,
including in-flight wall-cap cancellation, waits for contact.

## Ask the stranger for

1. The list (plain text, one item per line; or one file per item).
2. The criterion, in their words. Do not polish it — ambiguity in their
   phrasing is data.
3. The decision: how many do they actually need (top-k), or is it a
   full order?
4. Deadline / cost comfort, if any.

## Run

```
llmsort sort their-list.txt --by "their criterion verbatim" \
  --top-k K --max-dollars 0.50 --max-seconds 120 --scores
```

- Operator present for the whole run. **No unattended or cron execution
  of the CLI until in-flight cancellation lands** (the wall cap is
  checked between batches and can overshoot by one in-flight batch,
  including rate-limit cooldown waits — state turnaround to the
  stranger with that slack).
- Small list + expired-key class failures now name their cause on
  stderr; if a run degrades, the summary says why.

## Before revealing the tool's answer

Ask the stranger 3–5 boundary comparisons blind: pairs straddling the
top-k boundary ("which of these two is more X?"). Their answers are the
dependent variable (grand-plan Move 3: external validity, not judge
reproducibility). Record them before sending the packet.

## The packet (all CLI-emitted)

- The ordered list (top-k first), with `--scores` latents ± σ.
- The stderr summary verbatim: comparisons, cost, stop reason,
  `resolution:` line, rank risk.
- One honest sentence when the stop reason is `budget_exhausted` /
  `cost_budget_exhausted`: which adjacent ranks are unresolved at this
  budget.
- What it cost and how long it took.

## Afterward

- Score the boundary questions against the tool's order; log agreement.
- The week test: does this person send a materially new list,
  unassisted, within 7 days? That event, not the packet, is success.

## If the stop reason is budget_exhausted at the requested top-k

The default budget (4·n) is often not sized to the decision (measured
twice: top-3 on 8 and on 12 items both end unresolved). Rerun with
`--budget` at 2–4× the default before sending the packet, and say in
the packet which rerun produced the order. (Sizing the default to the
decision is frozen until contact — grand-plan §6.)

## Dry run (Feynman's test — executed 2026-08-29)

The flow had never been exercised end-to-end before. Self-made 12-item
list, criterion "likely to save a busy non-technical professional the
most hours per month in real use", exact command above with K=3,
gpt-5.4-mini: 48 comparisons, $0.0194, seconds of wall. Packet
contents produced exactly as specified — ordered list with latents ± σ,
stderr summary (stop `budget_exhausted`, `resolution: 11 of 11 adjacent
ranks within 1σ`, rank risk 5.6), so the honest sentence reads: the
top-3 boundary is unresolved at the default budget; a `--budget` rerun
is required before the packet ships. Boundary pairs extract naturally
(rank 3 vs 4, 2 vs 5). One cosmetic defect observed and frozen: the
`resolution:` hint suggests `--top-k` even when it is already set.
Artifacts: `/tmp/llmsort-consult/dryrun-{stdout,stderr}.txt`.
