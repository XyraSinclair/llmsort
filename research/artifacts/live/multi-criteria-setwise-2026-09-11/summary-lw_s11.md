## lw_s11 — n=40 k=8 overlap=2 rounds=1 repeats=2 model=google/gemma-4-31b-it seed=11

| arm | calls | ok | malformed | errored | in tok | out tok | $ | wall s |
|---|---|---|---|---|---|---|---|---|
| separate | 42 | 42 | 0 | 0 | 224340 | 378 | 0.0205 | 17 |
| joint | 14 | 14 | 0 | 0 | 77251 | 462 | 0.0083 | 8 |

| criterion | arm | parsed | flip | components |
|---|---|---|---|---|
| alpha | separate | 14 | 0.107 | 1 |
| novelty | separate | 14 | 0.107 | 1 |
| rigor | separate | 14 | 0.051 | 1 |
| alpha | joint | 14 | 0.087 | 1 |
| novelty | joint | 14 | 0.112 | 1 |
| rigor | joint | 14 | 0.107 | 1 |

| pair | arm | inter-criterion ρ |
|---|---|---|
| alpha~rigor | separate | +0.884 |
| novelty~alpha | separate | +0.825 |
| novelty~rigor | separate | +0.783 |
| alpha~rigor | joint | +0.959 |
| novelty~alpha | joint | +0.757 |
| novelty~rigor | joint | +0.811 |

| criterion | ρ(separate, joint) | top-10 overlap |
|---|---|---|
| novelty | +0.942 | 0.80 |
| alpha | +0.960 | 0.90 |
| rigor | +0.941 | 0.90 |

mean inter-criterion ρ inflation (joint − separate): +0.012
