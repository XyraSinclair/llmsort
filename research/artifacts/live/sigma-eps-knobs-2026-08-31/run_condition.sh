#!/bin/bash
# One sigma-eps condition: 3 fixed proverb pairs x 8 draws through the
# REAL 2p rail (judge --draws, evidence-rail path, nonce seam), terra.
# Usage: run_condition.sh <label>   (binary must already be built for the
# condition; the caller patches seriate.rs phase-1 reasoning and rebuilds)
set -euo pipefail
cd "$(cd "$(dirname "$0")/../../../.." && pwd)"  # repo root
label="$1"
out="research/artifacts/live/sigma-eps-knobs-2026-08-31/${label}.jsonl"
: > "$out"
run() {
  ./target/debug/llmsort judge "$1" "$2" --by "usefulness as advice" \
    --model "${SIGMA_EPS_MODEL:-openai/gpt-5.6-terra}" --template ratio_letter_2p_v1 \
    --draws 8 --json \
    | python3 -c "import json,sys; d=json.load(sys.stdin); d['pair']='$3'; print(json.dumps(d))" >> "$out"
}
run "measure twice, cut once" "don't put all your eggs in one basket" p1
run "premature optimization is the root of all evil" "if it ain't broke, don't fix it" p2
run "a bird in the hand is worth two in the bush" "a chain is only as strong as its weakest link" p3
python3 - "$out" <<'PY'
import json,sys,math
rows=[json.loads(l) for l in open(sys.argv[1])]
vars_=[]; costs=0; refusals=0
for r in rows:
    s=r["sigma_w"]; vars_.append(s*s); costs+=r["cost_nanodollars"]; refusals+=r["refusals"]
    print(f"  {r['pair']}: mean {r['mean']:+.3f} sigma_w {s:.3f} (n={r['comparisons']-r['refusals']})")
pooled=math.sqrt(sum(vars_)/len(vars_))
print(f"POOLED sigma_eps {pooled:.3f} nats/call · refusals {refusals} · ${costs/1e9:.3f}")
PY
