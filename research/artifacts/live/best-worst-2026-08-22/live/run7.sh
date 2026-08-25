#!/bin/bash
# E12 (folk baselines + truth anchors) + E13 (n=150 scaling), live.
# deepseek + luna; manifund n=24, anchors n=16, arxiv n=150; seed 17.
set -a; source ~/.config/scry-secrets/openrouter.env; set +a
export OPENROUTER_TIMEOUT_SECONDS=180 OPENROUTER_DISABLE_REASONING=1

cd ~/build/llmsort/research
BIN=../target/release/examples/setwise_cached
OUT=artifacts/live/best-worst-2026-08-22/live
MF="impact_per_dollar,theory_of_change,fit for a funder who wants cheap high-leverage AI safety field-building"
AX="methodological rigor,novelty of contribution,usefulness for a practitioner building LLM-based products"

run_model() {
  local m=$1 tag=$2
  # --- E12: manifund n=24 ---
  echo "=== mf-point-$tag $(date -u +%FT%TZ)"
  $BIN --answer point --ks 1 --n 24 --presentations 1 --seed 17 --model $m \
    --attrs "$MF" --spend-cap-usd 1.0 --out-dir $OUT/mf-point-$tag
  echo "=== mf-list24-$tag $(date -u +%FT%TZ)"
  $BIN --answer order --ks 24 --n 24 --presentations 1 --repeats 2 --seed 17 --model $m \
    --attrs "$MF" --skip-pairwise --spend-cap-usd 1.0 --out-dir $OUT/mf-list24-$tag
  # --- E12: anchors (truth) ---
  local pool attr slug
  for slug in countries rivers; do
    if [ $slug = countries ]; then attr="population"; else attr="length in kilometres"; fi
    pool=data/anchors_$slug.json
    echo "=== anch-$slug-ring-$tag $(date -u +%FT%TZ)"
    $BIN --answer order --ks 8 --design ring --presentations 1 --repeats 2 --n 16 --seed 17 \
      --model $m --corpus $pool --min-entity-chars 1 --attrs "$attr" \
      --spend-cap-usd 1.0 --out-dir $OUT/anch-$slug-ring-$tag
    echo "=== anch-$slug-point-$tag $(date -u +%FT%TZ)"
    $BIN --answer point --ks 1 --n 16 --presentations 1 --seed 17 \
      --model $m --corpus $pool --min-entity-chars 1 --attrs "$attr" \
      --skip-pairwise --spend-cap-usd 1.0 --out-dir $OUT/anch-$slug-point-$tag
    echo "=== anch-$slug-list16-$tag $(date -u +%FT%TZ)"
    $BIN --answer order --ks 16 --n 16 --presentations 1 --repeats 2 --seed 17 \
      --model $m --corpus $pool --min-entity-chars 1 --attrs "$attr" \
      --skip-pairwise --spend-cap-usd 1.0 --out-dir $OUT/anch-$slug-list16-$tag
  done
  # --- E13: arxiv n=150 ---
  echo "=== ax150-ring-$tag $(date -u +%FT%TZ)"
  $BIN --answer order --ks 8 --design ring --presentations 1 --repeats 2 --n 150 --seed 17 \
    --model $m --corpus data/arxiv_abstracts.json --entity-chars 1000 --attrs "$AX" \
    --spend-cap-usd 1.5 --out-dir $OUT/ax150-ring-$tag
  echo "=== ax150-point-$tag $(date -u +%FT%TZ)"
  $BIN --answer point --ks 1 --n 150 --presentations 1 --seed 17 \
    --model $m --corpus data/arxiv_abstracts.json --entity-chars 1000 --attrs "$AX" \
    --skip-pairwise --spend-cap-usd 1.0 --out-dir $OUT/ax150-point-$tag
}

export OPENROUTER_PROVIDER_JSON='{"order": ["parasail", "coreweave", "digitalocean", "streamlake", "sail-research"], "allow_fallbacks": false}'
run_model deepseek/deepseek-v4-flash deepseek 2>&1

unset OPENROUTER_PROVIDER_JSON
run_model openai/gpt-5.6-luna luna 2>&1
echo LIVE7_DONE
