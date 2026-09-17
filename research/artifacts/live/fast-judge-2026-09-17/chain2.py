"""One GPU borrow: warm-start the 0.6B scorer on corpus2 (+ lw-corpus replay), then bench it."""
import subprocess
PY = "/srv/build/lanced-slice/venv-gpu/bin/python"
C1 = "/srv/build/llmsort-bakeoff/research/artifacts/live/mc-teacher-corpus-2026-09-13"
cfg = ["--chunk", "40"]
subprocess.run([PY, "-u", "ft_scorer.py", "--out", "scorer-0.6b-v2", "--epochs", "1", "--eval-every", "1000", "--eval-lists", "198",
                "--init", "scorer-0.6b/latest.pt", "--corpus", C1 + ":150", "--corpus", "corpus2", "--checkpointing"] + cfg, check=True)
subprocess.run([PY, "-u", "bench_scorer.py", "--weights", "scorer-0.6b-v2/latest.pt", "--out", "scorer-0.6b-v2/bench.json"] + cfg)
open("scorer2.flag", "w").close()
