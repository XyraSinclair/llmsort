#!/bin/bash
# E6 robustness matrix — delimiter x entity-size x model x corpus.
# Env file (not committed): ~/.config/scry-secrets/openrouter.env
set -a; source ~/.config/scry-secrets/openrouter.env; set +a
export OPENROUTER_TIMEOUT_SECONDS=180

deepseek_env() {
  export OPENROUTER_DISABLE_REASONING=1
  export OPENROUTER_PROVIDER_JSON="{\"order\": [\"parasail\", \"coreweave\", \"digitalocean\", \"streamlake\", \"sail-research\"], \"allow_fallbacks\": false}"
}
open_env() {
  # Non-DeepSeek models: the provider pins are DeepSeek-specific — must be unset.
  unset OPENROUTER_PROVIDER_JSON
  export OPENROUTER_DISABLE_REASONING=1
}

cd ~/build/llmsort/research
BIN=../target/release/examples/setwise_cached
OUT=artifacts/live/best-worst-2026-08-22/live
MF_ATTRS="impact_per_dollar,theory_of_change,fit for a funder who wants cheap high-leverage AI safety field-building"
AX_ATTRS="methodological rigor,novelty of contribution,usefulness for a practitioner building LLM-based products"
DS=deepseek/deepseek-v4-flash

common() { echo "=== $1 $(date -u +%FT%TZ)"; }

# --- delimiter sweep (xml cell = order-ksweep-r2 k=8, same seed/m/r) ---
deepseek_env
common delim-bracket
$BIN --answer order --model $DS --ks 8 --n 24 --presentations 2 --repeats 2 --seed 17 --attrs "$MF_ATTRS" --delimiter bracket --spend-cap-usd 1.0 --out-dir $OUT/delim-bracket 2>&1
common delim-dash
$BIN --answer order --model $DS --ks 8 --n 24 --presentations 2 --repeats 2 --seed 17 --attrs "$MF_ATTRS" --delimiter dash --spend-cap-usd 1.0 --out-dir $OUT/delim-dash 2>&1

# --- entity-size sweep (1600 covered by prior runs) ---
common size-400
$BIN --answer order --model $DS --ks 8 --n 24 --presentations 2 --repeats 2 --seed 17 --attrs "$MF_ATTRS" --entity-chars 400 --spend-cap-usd 1.0 --out-dir $OUT/size-400 2>&1
common size-4800
$BIN --answer order --model $DS --ks 8 --n 24 --presentations 2 --repeats 2 --seed 17 --attrs "$MF_ATTRS" --entity-chars 4800 --spend-cap-usd 1.0 --out-dir $OUT/size-4800 2>&1
common size-8000
$BIN --answer order --model $DS --ks 8 --n 24 --presentations 2 --repeats 2 --seed 17 --attrs "$MF_ATTRS" --entity-chars 8000 --spend-cap-usd 1.0 --out-dir $OUT/size-8000 2>&1

# --- arXiv corpus, deepseek ---
common arxiv-deepseek
$BIN --answer order --model $DS --ks 8 --n 24 --presentations 2 --repeats 2 --seed 17 --attrs "$AX_ATTRS" --corpus data/arxiv_abstracts.json --entity-chars 1000 --spend-cap-usd 1.0 --out-dir $OUT/arxiv-deepseek 2>&1

# --- model sweep (pins unset) ---
open_env
common model-gpt41mini
$BIN --answer order --model openai/gpt-4.1-mini --ks 8 --n 24 --presentations 2 --repeats 2 --seed 17 --attrs "$MF_ATTRS" --spend-cap-usd 1.0 --out-dir $OUT/model-gpt41mini 2>&1
common model-gemini25flash
$BIN --answer order --model google/gemini-2.5-flash --ks 8 --n 24 --presentations 2 --repeats 2 --seed 17 --attrs "$MF_ATTRS" --spend-cap-usd 1.0 --out-dir $OUT/model-gemini25flash 2>&1
common arxiv-gpt41mini
$BIN --answer order --model openai/gpt-4.1-mini --ks 8 --n 24 --presentations 2 --repeats 2 --seed 17 --attrs "$AX_ATTRS" --corpus data/arxiv_abstracts.json --entity-chars 1000 --spend-cap-usd 1.0 --out-dir $OUT/arxiv-gpt41mini 2>&1

echo LIVE4_DONE
