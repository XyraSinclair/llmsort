## lw — n=40 k=8 overlap=2 rounds=1 repeats=2 model=google/gemma-4-31b-it seed=7

| arm | calls | ok | malformed | errored | in tok | out tok | $ | wall s |
|---|---|---|---|---|---|---|---|---|
| separate | 42 | 42 | 0 | 0 | 223212 | 378 | 0.0196 | 27 |
| joint | 14 | 14 | 0 | 0 | 76876 | 462 | 0.0076 | 5 |

| criterion | arm | parsed | flip | components |
|---|---|---|---|---|
| alpha | separate | 14 | 0.082 | 1 |
| novelty | separate | 14 | 0.138 | 1 |
| rigor | separate | 14 | 0.077 | 1 |
| alpha | joint | 14 | 0.092 | 1 |
| novelty | joint | 14 | 0.189 | 1 |
| rigor | joint | 14 | 0.117 | 1 |

| pair | arm | inter-criterion ρ |
|---|---|---|
| alpha~rigor | separate | +0.898 |
| novelty~alpha | separate | +0.854 |
| novelty~rigor | separate | +0.843 |
| alpha~rigor | joint | +0.952 |
| novelty~alpha | joint | +0.895 |
| novelty~rigor | joint | +0.882 |

| criterion | ρ(separate, joint) | top-10 overlap |
|---|---|---|
| novelty | +0.897 | 0.80 |
| alpha | +0.954 | 0.90 |
| rigor | +0.946 | 0.80 |

mean inter-criterion ρ inflation (joint − separate): +0.045
