"""One GPU borrow: time three scorer configs for 20 steps, train the fastest that fits for one epoch, bench it."""
import subprocess, re, os
PY = "/srv/build/lanced-slice/venv-gpu/bin/python"
cfgs = {"c40": ["--chunk", "40"], "c40ckpt": ["--chunk", "40", "--checkpointing"], "c40t768": ["--chunk", "40", "--max-tokens", "768"]}
res = {}
for k, a in cfgs.items():
    p = subprocess.run([PY, "ft_scorer.py", "--out", "probe-" + k, "--steps", "20", "--eval-lists", "1", "--eval-every", "1000"] + a, capture_output=True, text=True)
    open("probe.log", "a").write(f"### {k}\n" + p.stdout + p.stderr[-2000:])
    m = re.search(r"step 20/20 .* ([0-9.]+)s/step .* mem ([0-9.]+)G", p.stdout)
    res[k] = (float(m.group(1)), float(m.group(2))) if m else None
    print("probe", k, res[k], flush=True)
ok = sorted((v[0], k) for k, v in res.items() if v and v[1] < 28)
best = ok[0][1]; print("best", best, flush=True)
subprocess.run([PY, "-u", "ft_scorer.py", "--out", "scorer-0.6b", "--epochs", "1", "--eval-every", "900"] + cfgs[best])
subprocess.run([PY, "-u", "bench_scorer.py", "--weights", "scorer-0.6b/latest.pt", "--out", "scorer-0.6b/bench.json"] + [x for x in cfgs[best] if x != "--checkpointing"])
open("scorer.flag", "w").close()
