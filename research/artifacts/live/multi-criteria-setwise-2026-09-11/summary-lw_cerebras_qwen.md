## lw_cerebras_qwen — n=40 k=8 overlap=2 rounds=1 repeats=2 model=qwen-3.8-27b seed=7

| arm | calls | ok | malformed | errored | in tok | out tok | $ | wall s |
|---|---|---|---|---|---|---|---|---|
| separate | 42 | 33 | 0 | 9 | 173201 | 297 | 0.0000 | 29 |
| joint | 14 | 12 | 0 | 2 | 65891 | 396 | 0.0000 | 25 |

| criterion | arm | parsed | flip | components |
|---|---|---|---|---|
| alpha | separate | 10 | 0.000 | 1 |
| novelty | separate | 13 | 0.161 | 1 |
| rigor | separate | 10 | 0.060 | 1 |
| alpha | joint | 12 | 0.100 | 1 |
| novelty | joint | 12 | 0.100 | 1 |
| rigor | joint | 12 | 0.107 | 1 |

| pair | arm | inter-criterion ρ |
|---|---|---|
| alpha~rigor | separate | +0.869 |
| novelty~alpha | separate | +0.819 |
| novelty~rigor | separate | +0.713 |
| alpha~rigor | joint | +0.929 |
| novelty~alpha | joint | +0.910 |
| novelty~rigor | joint | +0.852 |

| criterion | ρ(separate, joint) | top-10 overlap |
|---|---|---|
| novelty | +0.928 | 0.90 |
| alpha | +0.957 | 0.80 |
| rigor | +0.956 | 0.80 |

mean inter-criterion ρ inflation (joint − separate): +0.096
