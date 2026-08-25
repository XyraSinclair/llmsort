#!/bin/bash
# E15 — degenerate pools: does the gauge stay one-sided when the corpus
# carries duplicates, paraphrase near-duplicates, or boilerplate stubs?
# Ring k=8 rounds=2 (the E13 recipe, run8 flags verbatim), seeds 17+18 for
# across-seed within-cluster stability, deepseek + luna, 3 arXiv attrs.
# One pairwise sigma-reading cell: dup variant, deepseek, seed 18, first
# attr only (compare per-item sigma vs run8's clean ax150 pairwise arm).
set -a; source ~/.config/scry-secrets/openrouter.env; set +a
export OPENROUTER_TIMEOUT_SECONDS=180 OPENROUTER_DISABLE_REASONING=1
cd ~/build/llmsort/research
BIN=../target/release/examples/setwise_cached
OUT=artifacts/live/e15-degenerate-2026-08-25/live
AX="methodological rigor,novelty of contribution,usefulness for a practitioner building LLM-based products"

ring() {
  local variant=$1 m=$2 tag=$3 seed=$4
  echo "=== e15-$variant-$tag-s$seed $(date -u +%FT%TZ)"
  $BIN --answer order --ks 8 --design ring --presentations 2 --repeats 2 --n 150 --seed $seed \
    --model $m --corpus data/arxiv150_$variant.json --entity-chars 1000 --min-entity-chars 1 \
    --attrs "$AX" --skip-pairwise --spend-cap-usd 1.0 --out-dir $OUT/e15-$variant-$tag-s$seed 2>&1
}

export OPENROUTER_PROVIDER_JSON='{"order": ["parasail", "coreweave", "digitalocean", "streamlake", "sail-research"], "allow_fallbacks": false}'
for v in dup para stub; do
  for s in 17 18; do ring $v deepseek/deepseek-v4-flash deepseek $s; done
done
echo "=== e15-dup-deepseek-pairwise-s18 $(date -u +%FT%TZ)"
$BIN --answer order --ks 8 --design ring --presentations 2 --repeats 2 --n 150 --seed 18 \
  --model deepseek/deepseek-v4-flash --corpus data/arxiv150_dup.json --entity-chars 1000 --min-entity-chars 1 \
  --attrs "methodological rigor" --spend-cap-usd 1.0 --out-dir $OUT/e15-dup-pairwise-deepseek-s18 2>&1

unset OPENROUTER_PROVIDER_JSON
for v in dup para stub; do
  for s in 17 18; do ring $v openai/gpt-5.6-luna luna $s; done
done
echo LIVE11_DONE
