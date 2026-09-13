## arxiv — n=40 k=8 overlap=2 rounds=1 repeats=2 model=google/gemma-4-31b-it seed=7

| arm | calls | ok | malformed | errored | in tok | out tok | $ | wall s |
|---|---|---|---|---|---|---|---|---|
| separate | 42 | 40 | 2 | 0 | 105107 | 580 | 0.0127 | 12 |
| joint | 14 | 13 | 1 | 0 | 37310 | 463 | 0.0044 | 9 |

| criterion | arm | parsed | flip | components |
|---|---|---|---|---|
| clarity | separate | 14 | 0.219 | 1 |
| evidence | separate | 12 | 0.157 | 1 |
| novelty | separate | 14 | 0.143 | 1 |
| clarity | joint | 14 | 0.265 | 1 |
| evidence | joint | 13 | 0.119 | 1 |
| novelty | joint | 14 | 0.194 | 1 |

| pair | arm | inter-criterion ρ |
|---|---|---|
| clarity~evidence | separate | -0.207 |
| novelty~clarity | separate | -0.009 |
| novelty~evidence | separate | +0.609 |
| clarity~evidence | joint | +0.015 |
| novelty~clarity | joint | -0.051 |
| novelty~evidence | joint | +0.655 |

| criterion | ρ(separate, joint) | top-10 overlap |
|---|---|---|
| novelty | +0.916 | 0.80 |
| clarity | +0.716 | 0.60 |
| evidence | +0.865 | 0.80 |

mean inter-criterion ρ inflation (joint − separate): +0.075
