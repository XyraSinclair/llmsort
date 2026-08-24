#!/bin/bash
set -a; source ~/.config/scry-secrets/openrouter.env; set +a
export OPENROUTER_DISABLE_REASONING=1
export OPENROUTER_TIMEOUT_SECONDS=180
export OPENROUTER_PROVIDER_JSON="{\"order\": [\"parasail\", \"coreweave\", \"digitalocean\", \"streamlake\", \"sail-research\"], \"allow_fallbacks\": false}"
cd ~/build/llmsort/research
BIN=../target/release/examples/setwise_cached
OUT=artifacts/live/best-worst-2026-08-22/live
ATTRS="impact_per_dollar,theory_of_change,fit for a funder who wants cheap high-leverage AI safety field-building"
echo "=== order k-sweep r2 $(date -u +%FT%TZ)"
$BIN --answer order --model deepseek/deepseek-v4-flash --ks 4,6,8,12 --n 24 --presentations 2 --repeats 2 --seed 17 --attrs "$ATTRS" --spend-cap-usd 1.0 --out-dir $OUT/order-ksweep-r2 2>&1
echo "=== bw k8 r2 $(date -u +%FT%TZ)"
$BIN --answer bw --model deepseek/deepseek-v4-flash --ks 8 --n 24 --presentations 2 --repeats 2 --seed 17 --attrs "$ATTRS" --spend-cap-usd 1.0 --out-dir $OUT/bw-k8-r2 2>&1
echo LIVE3_DONE
