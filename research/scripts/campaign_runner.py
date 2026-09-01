#!/usr/bin/env python3
"""Box-resident campaign runner: walk a JSON manifest of judging phases on
colo2 itself, so months of GPU work survive laptops, sessions, and restarts.

Each phase is a judge_and_land invocation. The runner is idempotent: every
phase runs with --resume-ledger, so a crash or restart re-buys nothing —
already-landed attributes are skipped from the ledger, not from local state.

A phase whose judge model is not served at its base_url is SKIPPED with a log
line (never silently): bring the serve up and re-run the campaign to activate
that lane. A phase whose attributes file does not exist yet (e.g. elaborated
forms still being authored) is likewise SKIPPED loudly.

Usage (on colo2):
  CARDINAL_BIN=~/llmsorting/target/release/cardinal \
  python3 scripts/campaign_runner.py campaigns/manifund-3mo.json

The runner exits 0 when every runnable phase is complete; run it under a
supervisor (systemd Restart=on-failure, or cron re-invocation) and it
reconciles from the ledger each time.
"""
import json, os, subprocess, sys, time, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
JL = os.path.join(HERE, "judge_and_land.py")


def model_served(base_url, model, api_key=None):
    try:
        req = urllib.request.Request(base_url.rstrip("/") + "/models")
        if api_key:
            req.add_header("Authorization", "Bearer " + api_key)
        with urllib.request.urlopen(req, timeout=10) as r:
            data = json.load(r)
        return any(m.get("id") == model for m in data.get("data", []))
    except Exception as e:
        print(f"  serve probe failed for {base_url}: {e}", flush=True)
        return False


def main():
    manifest_path = sys.argv[1]
    manifest = json.load(open(manifest_path))
    root = os.path.dirname(os.path.abspath(manifest_path))
    repo = os.path.dirname(root) if os.path.basename(root) == "campaigns" else root
    failures = 0
    for i, ph in enumerate(manifest["phases"]):
        tag = ph["run_tag"]
        print(f"=== phase {i+1}/{len(manifest['phases'])}: {tag}", flush=True)
        attrs = os.path.join(repo, ph["attributes"])
        if not os.path.exists(attrs):
            print(f"  SKIP: attributes file missing: {attrs}", flush=True)
            continue
        api_key = None
        if ph.get("api_key_env"):
            api_key = os.environ.get(ph["api_key_env"])
            if not api_key:
                print(f"  SKIP: api_key_env {ph['api_key_env']} not set in environment",
                      flush=True)
                continue
        if not model_served(ph["base_url"], ph["model"], api_key):
            print(f"  SKIP: model {ph['model']} not served at {ph['base_url']}",
                  flush=True)
            continue
        env = dict(os.environ)
        env["OPENROUTER_BASE_URL"] = ph["base_url"]
        if api_key:
            env["OPENROUTER_API_KEY"] = api_key
        else:
            env.setdefault("OPENROUTER_API_KEY", "local")
        for k, v in ph.get("env", {}).items():
            env[k] = str(v)
        cmd = [sys.executable, JL,
               "--corpus", os.path.join(repo, ph["corpus"]),
               "--attributes", attrs,
               "--model", ph["model"],
               "--budget", str(ph["budget"]),
               "--seed", str(ph.get("seed", 1)),
               "--run-tag", tag,
               "--outdir", ph.get("outdir", f"/tmp/campaign-{tag}"),
               "--resume-ledger"]
        if ph.get("template"):
            cmd += ["--template", ph["template"]]
        if ph.get("elaborate"):
            cmd += ["--elaborate"]
        if ph.get("concurrency"):
            cmd += ["--concurrency", str(ph["concurrency"])]
        if ph.get("no_cache"):
            cmd += ["--no-cache"]
        t = time.time()
        rc = subprocess.run(cmd, env=env, cwd=repo).returncode
        print(f"=== phase {tag}: exit {rc} after {time.time()-t:.0f}s", flush=True)
        if rc != 0:
            failures += 1
    if failures:
        sys.exit(1)  # supervisor restarts; --resume-ledger makes it cheap
    print("campaign complete: all runnable phases landed", flush=True)


if __name__ == "__main__":
    main()
