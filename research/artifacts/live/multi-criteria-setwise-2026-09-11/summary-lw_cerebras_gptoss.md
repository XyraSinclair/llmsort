## lw_cerebras_gptoss — n=40 k=8 overlap=2 rounds=1 repeats=2 model=gpt-oss-120b seed=7

| arm | calls | ok | malformed | errored | in tok | out tok | $ | wall s |
|---|---|---|---|---|---|---|---|---|
| separate | 42 | 42 | 0 | 0 | 219884 | 6367 | 0.0000 | 7 |
| joint | 14 | 14 | 0 | 0 | 75684 | 3985 | 0.0000 | 4 |

| criterion | arm | parsed | flip | components |
|---|---|---|---|---|
| alpha | separate | 14 | 0.128 | 1 |
| novelty | separate | 14 | 0.235 | 1 |
| rigor | separate | 14 | 0.194 | 1 |
| alpha | joint | 14 | 0.138 | 1 |
| novelty | joint | 14 | 0.204 | 1 |
| rigor | joint | 14 | 0.143 | 1 |

| pair | arm | inter-criterion ρ |
|---|---|---|
| alpha~rigor | separate | +0.817 |
| novelty~alpha | separate | +0.818 |
| novelty~rigor | separate | +0.746 |
| alpha~rigor | joint | +0.938 |
| novelty~alpha | joint | +0.937 |
| novelty~rigor | joint | +0.856 |

| criterion | ρ(separate, joint) | top-10 overlap |
|---|---|---|
| novelty | +0.909 | 0.80 |
| alpha | +0.947 | 0.90 |
| rigor | +0.879 | 0.70 |

mean inter-criterion ρ inflation (joint − separate): +0.117
