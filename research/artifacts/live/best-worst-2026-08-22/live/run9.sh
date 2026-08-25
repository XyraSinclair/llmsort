#!/bin/bash
# E14: the funnel — {setwise ring2, pointwise} screen -> pairwise top-10
# refinement of the top-30, n=150 arxiv, both models, 3 attributes.
set -a; source ~/.config/scry-secrets/openrouter.env; set +a
export OPENROUTER_TIMEOUT_SECONDS=180 OPENROUTER_DISABLE_REASONING=1
cd ~/build/llmsort/research
BIN=../target/release/examples/funnel_topk
OUT=artifacts/live/best-worst-2026-08-22/live/funnel
mkdir -p $OUT

declare -A ATTRS=(
  [rigor]="methodological rigor"
  [novelty]="novelty of contribution"
  [useful]="usefulness for a practitioner building LLM-based products"
)

run_model() {
  local m=$1 tag=$2
  for slug in rigor novelty useful; do
    for s1 in setwise point; do
      echo "=== funnel-$slug-$s1-$tag $(date -u +%FT%TZ)"
      $BIN "$m" $s1 "${ATTRS[$slug]}" $OUT/$slug-$s1-$tag.json 2>&1
    done
  done
}

export OPENROUTER_PROVIDER_JSON='{"order": ["parasail", "coreweave", "digitalocean", "streamlake", "sail-research"], "allow_fallbacks": false}'
run_model deepseek/deepseek-v4-flash deepseek
unset OPENROUTER_PROVIDER_JSON
run_model openai/gpt-5.6-luna luna
echo LIVE9_DONE
