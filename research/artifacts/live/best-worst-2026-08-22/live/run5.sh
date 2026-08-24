#!/bin/bash
# E6: gpt-5.6-luna cells — the model the search repo actually uses.
# manifund + repeat (pairwise test-retest band) + arXiv corpus.
set -a; source ~/.config/scry-secrets/openrouter.env; set +a
export OPENROUTER_TIMEOUT_SECONDS=180
export OPENROUTER_DISABLE_REASONING=1
unset OPENROUTER_PROVIDER_JSON   # pins are DeepSeek-specific

cd ~/build/llmsort/research
BIN=../target/release/examples/setwise_cached
OUT=artifacts/live/best-worst-2026-08-22/live
MF_ATTRS="impact_per_dollar,theory_of_change,fit for a funder who wants cheap high-leverage AI safety field-building"
AX_ATTRS="methodological rigor,novelty of contribution,usefulness for a practitioner building LLM-based products"
M=openai/gpt-5.6-luna

echo "=== model-gpt56luna $(date -u +%FT%TZ)"
$BIN --answer order --model $M --ks 8 --n 24 --presentations 2 --repeats 2 --seed 17 --attrs "$MF_ATTRS" --spend-cap-usd 1.0 --out-dir $OUT/model-gpt56luna 2>&1
echo "=== model-gpt56luna-rep2 $(date -u +%FT%TZ)"
$BIN --answer order --model $M --ks 8 --n 24 --presentations 2 --repeats 2 --seed 17 --attrs "$MF_ATTRS" --spend-cap-usd 1.0 --out-dir $OUT/model-gpt56luna-rep2 2>&1
echo "=== arxiv-gpt56luna $(date -u +%FT%TZ)"
$BIN --answer order --model $M --ks 8 --n 24 --presentations 2 --repeats 2 --seed 17 --attrs "$AX_ATTRS" --corpus data/arxiv_abstracts.json --entity-chars 1000 --spend-cap-usd 1.0 --out-dir $OUT/arxiv-gpt56luna 2>&1
echo LIVE5_DONE
