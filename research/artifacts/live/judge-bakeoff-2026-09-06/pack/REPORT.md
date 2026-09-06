# Judge bakeoff report

6 models, generated 2026-09-06T21:08:49.894036246+00:00

## Per-model battery

### Qwen/Qwen3.5-9B  (4320 calls, 768s, 5.6 calls/s, $14.147)

| lens | axis | retest ρ | wording b ρ | wording c ρ | slot bias (nats) | slot |m| | decisive |m| | par/A/B % | vis mass | logprob | refused | failed |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| lesswrong-posts | epistemic-rigor | +0.67 | +0.75 | +0.51 | +0.62 | 0.62 | 0.66 | 0/100/0 | 0.97 | 99% | 9 | 0 |
| lesswrong-posts | novelty-of-insight | +0.55 | +0.70 | +0.42 | +0.87 | 0.87 | 0.94 | 0/100/0 | 0.95 | 99% | 6 | 0 |
| lesswrong-posts | technical-alpha | +0.49 | +0.24 | +0.40 | +0.75 | 0.75 | 0.80 | 0/100/0 | 0.96 | 97% | 19 | 0 |
| manifund-proposals | epistemic-pollution-restraint | +0.84 | +0.73 | +0.76 | +0.57 | 0.57 | 0.66 | 0/100/0 | 0.98 | 96% | 26 | 0 |
| manifund-proposals | novel-world-expanding-hit | +0.88 | +0.65 | +0.42 | +0.85 | 0.85 | 0.92 | 0/100/0 | 0.97 | 97% | 19 | 0 |
| manifund-proposals | theory-of-change | +0.62 | +0.42 | +0.50 | +0.61 | 0.61 | 0.60 | 0/100/0 | 0.98 | 78% | 0 | 160 |

### deepseek/deepseek-v4-flash  (4320 calls, 1028s, 4.2 calls/s, $0.708)

| lens | axis | retest ρ | wording b ρ | wording c ρ | slot bias (nats) | slot |m| | decisive |m| | par/A/B % | vis mass | logprob | refused | failed |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| lesswrong-posts | epistemic-rigor | +0.11 | +0.40 | +0.17 | +0.09 | 0.13 | 0.14 | 14/70/16 | 0.96 | 97% | 22 | 0 |
| lesswrong-posts | novelty-of-insight | +0.31 | +0.39 | +0.43 | +0.04 | 0.13 | 0.16 | 17/52/31 | 0.96 | 93% | 49 | 0 |
| lesswrong-posts | technical-alpha | +0.41 | +0.51 | +0.14 | +0.11 | 0.29 | 0.27 | 10/54/37 | 0.92 | 76% | 176 | 0 |
| manifund-proposals | epistemic-pollution-restraint | +0.11 | -0.14 | +0.24 | +0.19 | 0.19 | 0.17 | 7/83/11 | 0.96 | 82% | 129 | 0 |
| manifund-proposals | novel-world-expanding-hit | +0.32 | +0.11 | +0.27 | +0.30 | 0.30 | 0.22 | 6/78/16 | 0.95 | 95% | 37 | 0 |
| manifund-proposals | theory-of-change | +0.30 | +0.42 | +0.28 | +0.20 | 0.20 | 0.16 | 12/84/5 | 0.96 | 97% | 20 | 0 |

### deepseek/deepseek-v4-pro  (2160 calls, 433s, 5.0 calls/s, $1.882)

| lens | axis | retest ρ | wording b ρ | wording c ρ | slot bias (nats) | slot |m| | decisive |m| | par/A/B % | vis mass | logprob | refused | failed |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| lesswrong-posts | epistemic-rigor | +0.38 |   n/a |   n/a | +0.93 | 1.13 | 1.32 | 0/98/2 | 0.82 | 92% | 0 | 0 |
| lesswrong-posts | novelty-of-insight | +0.12 |   n/a |   n/a | +0.72 | 1.12 | 1.52 | 1/92/8 | 0.76 | 92% | 0 | 0 |
| lesswrong-posts | technical-alpha | +0.59 |   n/a |   n/a | +0.54 | 1.01 | 1.30 | 2/81/16 | 0.75 | 92% | 0 | 0 |
| manifund-proposals | epistemic-pollution-restraint | +0.43 |   n/a |   n/a | +0.35 | 0.76 | 1.06 | 1/91/8 | 0.79 | 94% | 0 | 0 |
| manifund-proposals | novel-world-expanding-hit | +0.50 |   n/a |   n/a | +0.97 | 1.45 | 1.85 | 0/96/4 | 0.78 | 93% | 1 | 0 |
| manifund-proposals | theory-of-change | +0.67 |   n/a |   n/a | +0.79 | 0.90 | 1.12 | 1/97/3 | 0.81 | 93% | 0 | 0 |

### google/gemma-4-12b-it  (4320 calls, 1275s, 3.4 calls/s, $14.835)

| lens | axis | retest ρ | wording b ρ | wording c ρ | slot bias (nats) | slot |m| | decisive |m| | par/A/B % | vis mass | logprob | refused | failed |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| lesswrong-posts | epistemic-rigor | +0.45 | +0.18 | +0.14 | +0.05 | 0.05 | 0.04 | 28/68/4 | 1.00 | 100% | 1 | 0 |
| lesswrong-posts | novelty-of-insight | +0.80 | +0.64 | +0.57 | +0.00 | 0.01 | 0.02 | 59/26/15 | 1.00 | 91% | 68 | 0 |
| lesswrong-posts | technical-alpha | +0.90 | +0.63 | +0.65 | +0.01 | 0.02 | 0.02 | 57/29/14 | 1.00 | 88% | 87 | 0 |
| manifund-proposals | epistemic-pollution-restraint | +0.74 | +0.05 | +0.04 | +0.03 | 0.03 | 0.03 | 58/36/6 | 1.00 | 97% | 21 | 0 |
| manifund-proposals | novel-world-expanding-hit | +0.79 | +0.42 | +0.07 | +0.03 | 0.03 | 0.04 | 31/64/6 | 1.00 | 98% | 13 | 0 |
| manifund-proposals | theory-of-change | +0.67 | +0.09 | +0.38 | +0.03 | 0.04 | 0.04 | 21/70/9 | 1.00 | 100% | 3 | 0 |

### google/gemma-4-31b-it  (4320 calls, 280s, 15.4 calls/s, $1.482)

| lens | axis | retest ρ | wording b ρ | wording c ρ | slot bias (nats) | slot |m| | decisive |m| | par/A/B % | vis mass | logprob | refused | failed |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| lesswrong-posts | epistemic-rigor | +0.82 | +0.73 | +0.44 | +0.03 | 0.03 | 0.06 | 7/58/35 | 1.00 | 100% | 1 | 0 |
| lesswrong-posts | novelty-of-insight | +0.88 | +0.74 | +0.59 | -0.01 | 0.03 | 0.09 | 5/36/59 | 1.00 | 99% | 5 | 0 |
| lesswrong-posts | technical-alpha | +0.77 | +0.53 | +0.59 | +0.01 | 0.03 | 0.09 | 21/36/43 | 1.00 | 96% | 31 | 0 |
| manifund-proposals | epistemic-pollution-restraint | +0.64 | +0.58 | +0.46 | +0.01 | 0.02 | 0.04 | 54/42/4 | 1.00 | 90% | 72 | 0 |
| manifund-proposals | novel-world-expanding-hit | +0.80 | +0.48 | +0.49 | +0.06 | 0.06 | 0.11 | 7/58/35 | 1.00 | 99% | 3 | 0 |
| manifund-proposals | theory-of-change | +0.91 | +0.78 | +0.80 | +0.13 | 0.14 | 0.17 | 5/45/50 | 1.00 | 100% | 0 | 0 |

### qwen/qwen3.7-flash  (4320 calls, 542s, 8.0 calls/s, $0.401)

| lens | axis | retest ρ | wording b ρ | wording c ρ | slot bias (nats) | slot |m| | decisive |m| | par/A/B % | vis mass | logprob | refused | failed |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| lesswrong-posts | epistemic-rigor | +0.01 | +0.24 | -0.10 | +0.05 | 0.05 | 0.07 | 50/49/1 | 0.96 | 100% | 0 | 0 |
| lesswrong-posts | novelty-of-insight | +0.24 | +0.11 | +0.27 | +0.20 | 0.20 | 0.30 | 16/83/1 | 0.95 | 99% | 7 | 0 |
| lesswrong-posts | technical-alpha | +0.10 | +0.37 | +0.07 | +0.28 | 0.29 | 0.25 | 27/71/2 | 0.94 | 100% | 1 | 0 |
| manifund-proposals | epistemic-pollution-restraint | +0.45 | +0.31 | +0.14 | +0.16 | 0.16 | 0.24 | 11/89/0 | 0.97 | 100% | 0 | 0 |
| manifund-proposals | novel-world-expanding-hit | +0.66 | +0.37 | +0.47 | +0.24 | 0.24 | 0.30 | 5/94/1 | 0.97 | 100% | 0 | 0 |
| manifund-proposals | theory-of-change | -0.01 | +0.14 | +0.12 | +0.06 | 0.06 | 0.07 | 29/71/0 | 0.97 | 100% | 0 | 0 |

## Inter-model agreement (pair-level Spearman of signed log-ratios, wording a, draw 0, mean over cells)

| model | Qwen3.5-9B | deepseek-v4-flash | deepseek-v4-pro | gemma-4-12b-it | gemma-4-31b-it | qwen3.7-flash | consensus (LOO) | vs deepseek-v4-pro | vs gemma-4-31b-it |
|---|---|---|---|---|---|---|---|---|---|
| Qwen3.5-9B |   —  | -0.11 | -0.18 | -0.16 | -0.32 | +0.16 | -0.19 | -0.18 | -0.32 |
| deepseek-v4-flash | -0.11 |   —  | +0.05 | +0.11 | +0.14 | -0.06 | +0.03 | +0.05 | +0.14 |
| deepseek-v4-pro | -0.18 | +0.05 |   —  | +0.07 | +0.33 | -0.16 | -0.03 |   n/a | +0.33 |
| gemma-4-12b-it | -0.16 | +0.11 | +0.07 |   —  | +0.34 | -0.07 | +0.05 | +0.07 | +0.34 |
| gemma-4-31b-it | -0.32 | +0.14 | +0.33 | +0.34 |   —  | -0.19 | +0.13 | +0.33 |   n/a |
| qwen3.7-flash | +0.16 | -0.06 | -0.16 | -0.07 | -0.19 |   —  | -0.07 | -0.16 | -0.19 |

## Per-cell agreement with consensus (LOO)

| model | lesswrong-posts/epistemic-rigor | lesswrong-posts/novelty-of-insight | lesswrong-posts/technical-alpha | manifund-proposals/epistemic-pollution-restraint | manifund-proposals/novel-world-expanding-hit | manifund-proposals/theory-of-change |
|---|---|---|---|---|---|---|
| Qwen3.5-9B | -0.01 | -0.28 | -0.31 | -0.16 | -0.28 | -0.08 |
| deepseek-v4-flash | -0.04 | +0.05 | +0.10 | -0.02 | +0.13 | -0.06 |
| deepseek-v4-pro | -0.09 | +0.20 | +0.11 | -0.34 | -0.09 | +0.05 |
| gemma-4-12b-it | -0.03 | +0.15 | +0.43 | -0.07 | -0.06 | -0.14 |
| gemma-4-31b-it | +0.16 | +0.36 | +0.40 | -0.12 | -0.04 | +0.02 |
| qwen3.7-flash | -0.16 | -0.13 | -0.08 | +0.08 | -0.10 | -0.04 |
