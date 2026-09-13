## hn_comments — n=40 k=8 overlap=2 rounds=1 repeats=2 model=google/gemma-4-31b-it seed=7

| arm | calls | ok | malformed | errored | in tok | out tok | $ | wall s |
|---|---|---|---|---|---|---|---|---|
| separate | 42 | 42 | 0 | 0 | 54071 | 378 | 0.0051 | 4 |
| joint | 14 | 14 | 0 | 0 | 20109 | 462 | 0.0022 | 5 |

| criterion | arm | parsed | flip | components |
|---|---|---|---|---|
| civil | separate | 14 | 0.036 | 1 |
| concise | separate | 14 | 0.133 | 1 |
| informative | separate | 14 | 0.097 | 1 |
| civil | joint | 14 | 0.148 | 1 |
| concise | joint | 14 | 0.163 | 1 |
| informative | joint | 14 | 0.097 | 1 |

| pair | arm | inter-criterion ρ |
|---|---|---|
| civil~concise | separate | +0.442 |
| informative~civil | separate | +0.161 |
| informative~concise | separate | -0.120 |
| civil~concise | joint | +0.538 |
| informative~civil | joint | +0.017 |
| informative~concise | joint | -0.208 |

| criterion | ρ(separate, joint) | top-10 overlap |
|---|---|---|
| informative | +0.955 | 0.90 |
| civil | +0.932 | 0.90 |
| concise | +0.905 | 0.80 |

mean inter-criterion ρ inflation (joint − separate): -0.045
