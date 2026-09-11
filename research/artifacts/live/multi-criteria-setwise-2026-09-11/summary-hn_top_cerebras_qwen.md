## hn_top_cerebras_qwen — n=40 k=8 overlap=2 rounds=1 repeats=2 model=qwen-3.8-27b seed=7

| arm | calls | ok | malformed | errored | in tok | out tok | $ | wall s |
|---|---|---|---|---|---|---|---|---|
| separate | 42 | 42 | 0 | 0 | 64810 | 378 | 0.0000 | 2 |
| joint | 14 | 14 | 0 | 0 | 23554 | 462 | 0.0000 | 1 |

| criterion | arm | parsed | flip | components |
|---|---|---|---|---|
| actionable | separate | 14 | 0.107 | 1 |
| credible | separate | 14 | 0.209 | 1 |
| interesting | separate | 14 | 0.117 | 1 |
| actionable | joint | 14 | 0.173 | 1 |
| credible | joint | 14 | 0.189 | 1 |
| interesting | joint | 14 | 0.117 | 1 |

| pair | arm | inter-criterion ρ |
|---|---|---|
| credible~actionable | separate | -0.087 |
| interesting~actionable | separate | +0.016 |
| interesting~credible | separate | +0.620 |
| credible~actionable | joint | +0.254 |
| interesting~actionable | joint | +0.258 |
| interesting~credible | joint | +0.597 |

| criterion | ρ(separate, joint) | top-10 overlap |
|---|---|---|
| interesting | +0.890 | 0.80 |
| credible | +0.937 | 0.80 |
| actionable | +0.841 | 0.60 |

mean inter-criterion ρ inflation (joint − separate): +0.187
