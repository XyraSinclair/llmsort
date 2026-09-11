## hn_top — n=40 k=8 overlap=2 rounds=1 repeats=2 model=google/gemma-4-31b-it seed=7

| arm | calls | ok | malformed | errored | in tok | out tok | $ | wall s |
|---|---|---|---|---|---|---|---|---|
| separate | 42 | 41 | 1 | 0 | 67893 | 771 | 0.0067 | 12 |
| joint | 14 | 13 | 1 | 0 | 24536 | 463 | 0.0024 | 3 |

| criterion | arm | parsed | flip | components |
|---|---|---|---|---|
| actionable | separate | 13 | 0.119 | 1 |
| credible | separate | 14 | 0.235 | 1 |
| interesting | separate | 14 | 0.153 | 1 |
| actionable | joint | 13 | 0.101 | 1 |
| credible | joint | 14 | 0.158 | 1 |
| interesting | joint | 14 | 0.138 | 1 |

| pair | arm | inter-criterion ρ |
|---|---|---|
| credible~actionable | separate | -0.199 |
| interesting~actionable | separate | -0.050 |
| interesting~credible | separate | +0.473 |
| credible~actionable | joint | -0.345 |
| interesting~actionable | joint | +0.057 |
| interesting~credible | joint | +0.513 |

| criterion | ρ(separate, joint) | top-10 overlap |
|---|---|---|
| interesting | +0.929 | 0.80 |
| credible | +0.855 | 0.90 |
| actionable | +0.914 | 0.80 |

mean inter-criterion ρ inflation (joint − separate): +0.001
