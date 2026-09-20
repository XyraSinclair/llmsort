"""Launch the window-size sweep for both judges. python3 judges_run.py   (idempotent: finished outputs are skipped)

gemma-4-31b (OpenRouter, via the prebuilt llmsort binary's setwise design): 3 cohorts x 12 attributes x k in {4, 8, 24}
x seeds {1, 2} -> gemma/<cohort>-a<ai>-k<k>-s<seed>.json. About $0.004 a sort.
Hosted Jev: step.py <cohort> elab 3 <k> for k in {2, 4, 12, 24} (k=8 already run) -> obs-/rho-<cohort>-elab-k<k>.json.
"""
import json, os, subprocess, sys
from concurrent.futures import ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
LLMSORT = os.environ.get("LLMSORT", os.path.join(HERE, "../../../../target/release/llmsort"))
ATTRS = json.load(open(f"{HERE}/attributes.json"))
COHORTS = ("arxiv", "manifund", "lw")


def gemma(cohort):
    for k in (4, 8, 24):
        for seed in (1, 2):
            for ai, a in enumerate(ATTRS):
                out = f"{HERE}/gemma/{cohort}-a{ai}-k{k}-s{seed}.json"
                if os.path.exists(out) and os.path.getsize(out) > 100:
                    continue
                r = subprocess.run([LLMSORT, "sort", f"{HERE}/gemma/{cohort}.items.json", "--by", a["text"], "--model", "google/gemma-4-31b-it",
                                    "--setwise", "--k", str(k), "--seed", str(seed), "--concurrency", "6", "--format", "json", "--scores", "--quiet"],
                                   capture_output=True, text=True)
                if r.returncode == 0 and r.stdout.strip().startswith("{"):
                    open(out, "w").write(r.stdout)
                else:
                    print(f"FAIL gemma {cohort} a{ai} k{k} s{seed}: {r.stderr.strip()[-200:]}", flush=True)
    print(f"gemma {cohort} done", flush=True)


def jev():
    for k in (2, 4, 12, 24):
        for cohort in COHORTS:
            if os.path.exists(f"{HERE}/rho-{cohort}-elab-k{k}.json"):
                continue
            r = subprocess.run(["xyra-vault", "run", os.path.expanduser("~/x/jev/typed-judgment"), "--", "python3", f"{HERE}/step.py", cohort, "elab", "3", str(k)],
                               capture_output=True, text=True)
            print(f"jev {cohort} k{k} exit {r.returncode}: " + (r.stdout.strip().splitlines()[-1] if r.stdout.strip() else r.stderr.strip()[-300:]), flush=True)


with ThreadPoolExecutor(4) as ex:
    fs = [ex.submit(gemma, c) for c in COHORTS] + [ex.submit(jev)]
    for f in fs:
        f.result()
print("all done", flush=True)
