## lw_cerebras_qwen_indep — n=40 k=8 overlap=2 rounds=1 repeats=2 model=qwen-3.8-27b seed=7

| arm | calls | ok | malformed | errored | in tok | out tok | $ | wall s |
|---|---|---|---|---|---|---|---|---|
| separate | 42 | 42 | 0 | 0 | 221634 | 378 | 0.0000 | 17 |
| joint | 14 | 11 | 0 | 3 | 61048 | 363 | 0.0000 | 23 |

| criterion | arm | parsed | flip | components |
|---|---|---|---|---|
| alpha | separate | 14 | 0.066 | 1 |
| novelty | separate | 14 | 0.153 | 1 |
| rigor | separate | 14 | 0.051 | 1 |
| alpha | joint | 11 | 0.080 | 1 |
| novelty | joint | 11 | 0.125 | 1 |
| rigor | joint | 11 | 0.116 | 1 |

| pair | arm | inter-criterion ρ |
|---|---|---|
| alpha~rigor | separate | +0.903 |
| novelty~alpha | separate | +0.816 |
| novelty~rigor | separate | +0.705 |
| alpha~rigor | joint | +0.870 |
| novelty~alpha | joint | +0.863 |
| novelty~rigor | joint | +0.835 |

| criterion | ρ(separate, joint) | top-10 overlap |
|---|---|---|
| novelty | +0.923 | 0.90 |
| alpha | +0.981 | 1.00 |
| rigor | +0.924 | 0.80 |

mean inter-criterion ρ inflation (joint − separate): +0.048
