## hn_comments — n=40 k=8 overlap=2 rounds=1 repeats=2 model=google/gemma-4-31b-it seed=7

| arm | calls | ok | malformed | errored | in tok | out tok | $ | wall s |
|---|---|---|---|---|---|---|---|---|
| separate | 42 | 42 | 0 | 0 | 54082 | 378 | 0.0059 | 10 |
| joint | 14 | 14 | 0 | 0 | 20109 | 462 | 0.0024 | 11 |

| criterion | arm | parsed | flip | components |
|---|---|---|---|---|
| civil | separate | 14 | 0.061 | 1 |
| funny | separate | 14 | 0.153 | 1 |
| informative | separate | 14 | 0.087 | 1 |
| civil | joint | 14 | 0.087 | 1 |
| funny | joint | 14 | 0.184 | 1 |
| informative | joint | 14 | 0.087 | 1 |

| pair | arm | inter-criterion ρ |
|---|---|---|
| funny~civil | separate | -0.566 |
| funny~informative | separate | -0.315 |
| informative~civil | separate | +0.192 |
| funny~civil | joint | -0.451 |
| funny~informative | joint | -0.185 |
| informative~civil | joint | -0.036 |

| criterion | ρ(separate, joint) | top-10 overlap |
|---|---|---|
| funny | +0.763 | 0.70 |
| informative | +0.948 | 0.90 |
| civil | +0.891 | 0.70 |

mean inter-criterion ρ inflation (joint − separate): +0.005
