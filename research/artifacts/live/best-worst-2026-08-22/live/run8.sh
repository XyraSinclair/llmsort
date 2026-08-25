#!/bin/bash
# E13 close-out: does structural density (rounds=2) recover the n=150 drop,
# and what is the pairwise test-retest ceiling at n=150? Seed 18 (planner +
# presentation reshuffle; entity set unchanged — all 150 corpus items are
# eligible), so the in-run pairwise arm doubles as the seed-17 retest.
set -a; source ~/.config/scry-secrets/openrouter.env; set +a
export OPENROUTER_TIMEOUT_SECONDS=180 OPENROUTER_DISABLE_REASONING=1
cd ~/build/llmsort/research
BIN=../target/release/examples/setwise_cached
OUT=artifacts/live/best-worst-2026-08-22/live
AX="methodological rigor,novelty of contribution,usefulness for a practitioner building LLM-based products"

export OPENROUTER_PROVIDER_JSON='{"order": ["parasail", "coreweave", "digitalocean", "streamlake", "sail-research"], "allow_fallbacks": false}'
echo "=== ax150-ring2-deepseek $(date -u +%FT%TZ)"
$BIN --answer order --ks 8 --design ring --presentations 2 --repeats 2 --n 150 --seed 18 \
  --model deepseek/deepseek-v4-flash --corpus data/arxiv_abstracts.json --entity-chars 1000 \
  --attrs "$AX" --spend-cap-usd 1.5 --out-dir $OUT/ax150-ring2-deepseek 2>&1

unset OPENROUTER_PROVIDER_JSON
echo "=== ax150-ring2-luna $(date -u +%FT%TZ)"
$BIN --answer order --ks 8 --design ring --presentations 2 --repeats 2 --n 150 --seed 18 \
  --model openai/gpt-5.6-luna --corpus data/arxiv_abstracts.json --entity-chars 1000 \
  --attrs "$AX" --spend-cap-usd 1.5 --out-dir $OUT/ax150-ring2-luna 2>&1
echo LIVE8_DONE
