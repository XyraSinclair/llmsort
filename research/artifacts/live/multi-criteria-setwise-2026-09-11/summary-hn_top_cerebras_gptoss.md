## hn_top_cerebras_gptoss — n=40 k=8 overlap=2 rounds=1 repeats=2 model=gpt-oss-120b seed=7

| arm | calls | ok | malformed | errored | in tok | out tok | $ | wall s |
|---|---|---|---|---|---|---|---|---|
| separate | 42 | 42 | 0 | 0 | 63894 | 5637 | 0.0000 | 5 |
| joint | 14 | 14 | 0 | 0 | 23146 | 3134 | 0.0000 | 2 |

| criterion | arm | parsed | flip | components |
|---|---|---|---|---|
| actionable | separate | 14 | 0.179 | 1 |
| credible | separate | 14 | 0.209 | 1 |
| interesting | separate | 14 | 0.138 | 1 |
| actionable | joint | 14 | 0.173 | 1 |
| credible | joint | 14 | 0.306 | 1 |
| interesting | joint | 14 | 0.194 | 1 |

| pair | arm | inter-criterion ρ |
|---|---|---|
| credible~actionable | separate | +0.160 |
| interesting~actionable | separate | -0.060 |
| interesting~credible | separate | +0.552 |
| credible~actionable | joint | +0.141 |
| interesting~actionable | joint | +0.029 |
| interesting~credible | joint | +0.746 |

| criterion | ρ(separate, joint) | top-10 overlap |
|---|---|---|
| interesting | +0.852 | 0.70 |
| credible | +0.802 | 0.60 |
| actionable | +0.824 | 0.70 |

mean inter-criterion ρ inflation (joint − separate): +0.088
