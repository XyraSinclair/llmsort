"""Teacher pass over corpus2 lists: one joint m=3 setwise run per list (k=8, overlap 2, 2 presentations),
gemma-4-31b-it via OpenRouter, resumable (skips lists with a summary), N lists in flight.
usage: run_corpus2.py <domain> [--parallel 6]   (cwd = corpus2 dir; key in ../.env.openrouter)"""
import argparse, json, os, subprocess, sys, time
from concurrent.futures import ThreadPoolExecutor
BIN = "/srv/build/llmsort-bakeoff/target/debug/examples/multi_criteria_setwise"
ap = argparse.ArgumentParser(); ap.add_argument("domain"); ap.add_argument("--parallel", type=int, default=6); a = ap.parse_args()
for line in open("../.env.openrouter"):
    k, _, v = line.strip().partition("="); os.environ[k] = v
coh = [c for c in json.load(open("cohorts.json")) if c["shard"] == a.domain]
out = f"runs/{a.domain}"; os.makedirs(out, exist_ok=True)
todo = [c for c in coh if not os.path.exists(f"{out}/summary-{c['list_id']}.json")]
print(f"{a.domain}: {len(coh)} lists, {len(todo)} to run, parallel {a.parallel}", flush=True)
def run(c):
    L = c["list_id"]; crit = ";".join(f"{k['name']}={k['text']}" for k in c["criteria"])
    assert all(";" not in k["text"] and "=" not in k["text"] for k in c["criteria"])
    cmd = [BIN, "--arm", "joint", "--k", "8", "--overlap", "2", "--rounds", "1", "--repeats", "2",
           "--model", "google/gemma-4-31b-it", "--base-url", "https://openrouter.ai/api/v1",
           "--items", f"lists/{a.domain}/{L}.json", "--label", L, "--criteria", crit, "--seed", str(c["seed"]),
           "--out", out, "--concurrency", "4", "--max-chars", "3000"]
    t = time.time()
    with open(f"{out}/log-{L}.txt", "w") as lg: rc = subprocess.call(cmd, stdout=lg, stderr=subprocess.STDOUT)
    cost = mal = None
    try:
        s = json.load(open(f"{out}/summary-{L}.json")); arm = s["arms"][0]; cost, mal = arm["cost_dollars"], arm["malformed"]
    except Exception as e: cost = f"nosummary:{e.__class__.__name__}"
    return f"done {L} rc={rc} cost={cost} malformed={mal} {time.time()-t:.0f}s"
t0 = time.time(); n = 0; spent = 0.0
with ThreadPoolExecutor(a.parallel) as ex:
    for msg in ex.map(run, todo):
        n += 1
        try: spent += float(msg.split("cost=")[1].split()[0])
        except ValueError: pass
        print(msg, f"| {n}/{len(todo)} ${spent:.2f} {(time.time()-t0)/60:.1f}m", flush=True)
print(f"{a.domain} finished: {n} lists, ${spent:.2f}, {(time.time()-t0)/60:.1f}m", flush=True)
