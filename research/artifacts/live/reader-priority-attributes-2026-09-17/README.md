# Reader-priority attributes: twelve candidates rated against the existing battery

Run 2026-09-17 14:05–14:20 PT. Twelve candidate attributes for "what should a human read
first", written from the ideonomy method (the assumptions the existing 2,200-attribute battery
makes about the reader — text-only value, infinite reader time, linear full reading, no reading
cost, no dependency between texts; Gunkel's importance-detectors: read what matters off a cost
paid, not a claim made), rated by the same teacher (`google/gemma-4-31b-it`, joint setwise,
n=20, k=8, overlap 2, 2 presentations) on the same 20 entities per lens the existing battery was
scored on, so differentiation is measured, not asserted. Reading and verdicts in
`../../../notes/reader-priority-attributes-2026-09-17.md`.

## Files

- `cand.criteria.json` — the twelve prompts, exact text.
- `cohorts.json` — 12 lists: P/C/M (lesswrong-posts / lesswrong-comments / manifund-proposals)
  × T1–T4 (triples 1–3, 4–6, 7–9, 10–12), each over the lens's 20 entities.
- `lists.tgz` — `lists/cand/<list>.json` item texts (`[{"id","text"}]`, full entity text).
- `ledger.jsonl.gz` — 720 rows `{list_id,item_id,criterion_name,criterion_text,latent,std,flip,parsed,judge,seed}`.
- `teacher-cand.log` — 12 lists, 72 calls, $0.07, 0.3 min, 0 malformed.
- `existing-axes/<lens>.tsv.gz` — the existing battery's gemma-4-31b latents on the same
  entities: `axis_key entity_id latent_mean latent_std run_id` (714 / 555 / 726 axes).
- `scripts/analyze.py` → `analysis.txt`, `analysis.json`: per candidate and lens — teacher flip,
  r to regret-if-missed, max |r| to any existing axis, R² on the existing battery's top 4 and
  top-k (80% variance) principal components, nearest candidate.
- `scripts/null.py` → `null.txt`: permutation nulls for max |r| and R² (with 700 axes and 20
  items the max |r| to *some* axis is .64 by chance, .75 at the 95th percentile — the max-|r|
  column is uninformative; R² on 4 PCs has null median .20, 95th .44), and leave-one-out
  predictive r of regret-if-missed from the existing PCs alone, each candidate alone, and both.
- `scripts/neighbors.py` → `neighbors.txt`: r to the semantically nearest existing families
  (composite and best single wording) where those families were rated on the same entities.

- `pass2/` — the second pass (14:25 PT, 6 lists, $0.03, 0 malformed): the three attributes the
  sharpening reader named (reading-pleasure, mistake-prevention, challenge-to-priors) and
  reader-empowerment, source-uniqueness, misleading-if-trusted re-rated in a different triple as
  a repeat-reliability read; `cohorts.json`, `cand2.criteria.json`, `ledger.jsonl`,
  `teacher-cand2.log`; `scripts/analyze2.py` → `pass2/analysis2.txt` (flip, repeat r to pass 1,
  R² on the battery's 4 PCs, nearest pass-1/pass-2 candidates and families, top/bottom posts).
- `existing-axes/lw-entities.tsv` — the 20 post ids with the first 160 chars of text.

Scripts run from this directory with numpy only.
