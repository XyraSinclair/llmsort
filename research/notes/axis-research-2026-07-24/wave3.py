#!/usr/bin/env python3
"""Wave-3 probe: run + verdict in one resumable script (llmsort binary;
the cardinal harness of waves 1-2 is retired).

Method and frozen thresholds identical to WAVE2_SPEC.md: 12-item
decoy-planted sets under wave3/, primary decoy always line 2 (0-based
index 1), trio opus46/gpt56sol/mini54, 24-comparison budget.
T1: opus<->sol Spearman >= 0.60. T2: (fr-fr) - mean(fr-mini) >= 0.20.
T3: decoy rank >= 7 for both frontiers AND mini places it >= 3 ranks
higher than the frontiers' best rank for it.
PASS = T1 and (T2 or T3); WEAK = T1 only; FAIL = not T1.
"""
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).parent
W3 = HERE / "wave3"
LLMSORT = Path.home() / "projects/llmsort/target/release/llmsort"
MODELS = {
    "opus46": "anthropic/claude-opus-4.6",
    "gpt56sol": "openai/gpt-5.6-sol",
    "mini54": "openai/gpt-5.4-mini",
}
DECOY = 1  # 0-based line index of the primary decoy in every probe file


def run() -> int:
    prompts = json.loads((W3 / "prompts.json").read_text())
    failures = 0
    for axis_key, wording in prompts.items():
        probe = W3 / f"{axis_key}.txt"
        for model_key, model_slug in MODELS.items():
            out = W3 / f"sort-{axis_key}-{model_key}.json"
            if out.exists():
                print(f"skip {out.name}", flush=True)
                continue
            cmd = [
                str(LLMSORT), "sort", str(probe),
                "--by", wording,
                "--model", model_slug,
                "--budget", "24",
                "--format", "json",
                "--scores",
            ]
            print(f"RUN {axis_key} x {model_key}", flush=True)
            proc = subprocess.run(cmd, capture_output=True, text=True)
            if proc.returncode != 0:
                print(f"FAIL {axis_key} x {model_key}: {proc.stderr[-1500:]}",
                      flush=True)
                failures += 1
                continue
            out.write_text(proc.stdout)
            print(f"OK -> {out.name}", flush=True)
    return failures


def ranks(axis, model):
    data = json.loads((W3 / f"sort-{axis}-{model}.json").read_text())
    return {int(it["id"].split("-")[1]): it["rank"] for it in data["items"]}


def spearman(ra, rb):
    n = len(ra)
    d2 = sum((ra[i] - rb[i]) ** 2 for i in ra)
    return 1 - 6 * d2 / (n * (n * n - 1))


def analyze() -> None:
    prompts = json.loads((W3 / "prompts.json").read_text())
    print(f"\n{'axis':24s} {'fr-fr':>6s} {'op-mi':>6s} {'so-mi':>6s} "
          f"{'gap':>6s} {'decoy fr/mi':>11s}  verdict")
    for axis in prompts:
        try:
            R = {m: ranks(axis, m) for m in MODELS}
        except FileNotFoundError:
            print(f"{axis:24s} (runs missing)")
            continue
        frfr = spearman(R["opus46"], R["gpt56sol"])
        opmi = spearman(R["opus46"], R["mini54"])
        somi = spearman(R["gpt56sol"], R["mini54"])
        gap = frfr - (opmi + somi) / 2
        d_fr_best = min(R["opus46"][DECOY], R["gpt56sol"][DECOY])
        d_mi = R["mini54"][DECOY]
        t1 = frfr >= 0.60
        t2 = gap >= 0.20
        t3 = (min(R["opus46"][DECOY], R["gpt56sol"][DECOY]) >= 7
              and d_fr_best - d_mi >= 3)
        verdict = "PASS" if (t1 and (t2 or t3)) else ("WEAK" if t1 else "FAIL")
        print(f"{axis:24s} {frfr:+.3f} {opmi:+.3f} {somi:+.3f} "
              f"{gap:+.3f} {d_fr_best:>4d}/{d_mi:<4d}  {verdict} "
              f"(T1={int(t1)} T2={int(t2)} T3={int(t3)})")
        for m in MODELS:
            order = sorted(R[m], key=lambda i: R[m][i])
            print(f"  {m:10s} order (1-based items): "
                  f"{[i + 1 for i in order]}")


if __name__ == "__main__":
    failures = run()
    analyze()
    sys.exit(1 if failures else 0)
