#!/bin/bash
# E11: ring vs disjoint chunk design, live — can one overlapping round
# replace two disjoint rounds? deepseek + luna, manifund pool, seed 17.
set -a; source ~/.config/scry-secrets/openrouter.env; set +a
export OPENROUTER_TIMEOUT_SECONDS=180 OPENROUTER_DISABLE_REASONING=1

cd ~/build/llmsort/research
BIN=../target/release/examples/setwise_cached
OUT=artifacts/live/best-worst-2026-08-22/live
MF_ATTRS="impact_per_dollar,theory_of_change,fit for a funder who wants cheap high-leverage AI safety field-building"

export OPENROUTER_PROVIDER_JSON="{\"order\": [\"parasail\", \"coreweave\", \"digitalocean\", \"streamlake\", \"sail-research\"], \"allow_fallbacks\": false}"
DS=deepseek/deepseek-v4-flash
echo "=== ring-m1r2-deepseek $(date -u +%FT%TZ)"
$BIN --answer order --model $DS --ks 8 --n 24 --design ring --presentations 1 --repeats 2 --seed 17 --attrs "$MF_ATTRS" --spend-cap-usd 1.0 --out-dir $OUT/ring-m1r2-deepseek 2>&1
echo "=== ring-m1r1-deepseek $(date -u +%FT%TZ)"
$BIN --answer order --model $DS --ks 8 --n 24 --design ring --presentations 1 --repeats 1 --seed 17 --attrs "$MF_ATTRS" --spend-cap-usd 1.0 --out-dir $OUT/ring-m1r1-deepseek 2>&1

unset OPENROUTER_PROVIDER_JSON
LUNA=openai/gpt-5.6-luna
echo "=== ring-m1r2-luna $(date -u +%FT%TZ)"
$BIN --answer order --model $LUNA --ks 8 --n 24 --design ring --presentations 1 --repeats 2 --seed 17 --attrs "$MF_ATTRS" --spend-cap-usd 1.0 --out-dir $OUT/ring-m1r2-luna 2>&1
echo LIVE6_DONE
