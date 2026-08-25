#!/bin/bash
# E7: PMF/logprob arm for the order instrument — twins of E11 ring-m1r2
# (manifund n=24, seed 17) and run8 ax150-ring2 (n=150, seed 18), with
# --logprobs; baselines live in the twins, so --skip-pairwise throughout.
set -a; source ~/.config/scry-secrets/openrouter.env; set +a
export OPENROUTER_TIMEOUT_SECONDS=180 OPENROUTER_DISABLE_REASONING=1
cd ~/build/llmsort/research
BIN=../target/release/examples/setwise_cached
OUT=artifacts/live/best-worst-2026-08-22/live
MF="impact_per_dollar,theory_of_change,fit for a funder who wants cheap high-leverage AI safety field-building"
AX="methodological rigor,novelty of contribution,usefulness for a practitioner building LLM-based products"

run_model() {
  local m=$1 tag=$2
  echo "=== lp-mf-ring-$tag $(date -u +%FT%TZ)"
  $BIN --answer order --ks 8 --design ring --presentations 1 --repeats 2 --n 24 --seed 17 \
    --model $m --attrs "$MF" --logprobs --skip-pairwise \
    --spend-cap-usd 1.0 --out-dir $OUT/lp-mf-ring-$tag 2>&1
  echo "=== lp-ax150-ring2-$tag $(date -u +%FT%TZ)"
  $BIN --answer order --ks 8 --design ring --presentations 2 --repeats 2 --n 150 --seed 18 \
    --model $m --corpus data/arxiv_abstracts.json --entity-chars 1000 --attrs "$AX" \
    --logprobs --skip-pairwise --spend-cap-usd 1.5 --out-dir $OUT/lp-ax150-ring2-$tag 2>&1
}

export OPENROUTER_PROVIDER_JSON='{"order": ["parasail", "coreweave", "digitalocean", "streamlake", "sail-research"], "allow_fallbacks": false}'
run_model deepseek/deepseek-v4-flash deepseek
unset OPENROUTER_PROVIDER_JSON
run_model openai/gpt-5.6-luna luna
echo LIVE10_DONE
