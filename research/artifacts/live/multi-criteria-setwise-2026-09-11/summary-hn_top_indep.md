## hn_top_indep — n=40 k=8 overlap=2 rounds=1 repeats=2 model=google/gemma-4-31b-it seed=7

| arm | calls | ok | malformed | errored | in tok | out tok | $ | wall s |
|---|---|---|---|---|---|---|---|---|
| separate | 42 | 42 | 0 | 0 | 67893 | 380 | 0.0058 | 12 |
| joint | 14 | 13 | 1 | 0 | 25542 | 463 | 0.0026 | 4 |

| criterion | arm | parsed | flip | components |
|---|---|---|---|---|
| actionable | separate | 14 | 0.102 | 1 |
| credible | separate | 14 | 0.189 | 1 |
| interesting | separate | 14 | 0.138 | 1 |
| actionable | joint | 13 | 0.095 | 1 |
| credible | joint | 14 | 0.189 | 1 |
| interesting | joint | 14 | 0.117 | 1 |

| pair | arm | inter-criterion ρ |
|---|---|---|
| credible~actionable | separate | -0.264 |
| interesting~actionable | separate | -0.069 |
| interesting~credible | separate | +0.469 |
| credible~actionable | joint | -0.310 |
| interesting~actionable | joint | -0.028 |
| interesting~credible | joint | +0.649 |

| criterion | ρ(separate, joint) | top-10 overlap |
|---|---|---|
| interesting | +0.914 | 0.80 |
| credible | +0.864 | 0.70 |
| actionable | +0.925 | 0.80 |

mean inter-criterion ρ inflation (joint − separate): +0.059
