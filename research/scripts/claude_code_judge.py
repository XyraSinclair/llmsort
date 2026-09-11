#!/usr/bin/env python3
"""claude_code_judge — structured LLM judgments through Claude Code.

Elicits one schema-validated judgment per invocation through `claude -p
--json-schema`, billed to the operator's Claude subscription instead of
a metered API key. Reads a prompt (stdin or file), prints the validated
object as one compact JSON line on stdout; provenance (served model,
latency, tokens) as one JSON line on stderr.

  claude_code_judge.py --schema schema.json [prompt.md]
  echo "..." | claude_code_judge.py --schema '{"type":"object",...}'

Exit codes: 0 ok · 2 quota/session-limit (pause; retry after the reset
the CLI names) · 1 anything else. Transient failures retry (--retries).

Why this exists for llmsorting: the engine treats judgments as
noisy measurements; this is an elicitation channel with zero marginal
API cost. The `structured_output` field of the print-mode JSON envelope
carries the schema-validated object — the harness enforces the schema
server-side (model retries on mismatch), so no output parsing is needed.

--pure mode (recommended for measurement): runs each judgment in a
scratch CLAUDE_CONFIG_DIR with a minimal --system-prompt, so no user
CLAUDE.md, memory, hooks, skills, or MCP servers reach the judge —
personal context is a framing contaminant for exactly the reasons this
project prices. Verified by in-context probe (2026-07-29, claude CLI
2.1.220): context drops ~40k -> ~18k tokens; the remainder is the
Claude Code harness core plus tool definitions, which subscription
OAuth cannot strip (--bare and --setting-sources both drop the OAuth
credential source and yield "Not logged in").

Auth for the scratch dir (macOS): Claude Code keys its Keychain item by
config-dir hash — service "Claude Code-credentials-<sha256(dir)[:8]>".
The main item is mirrored to that name fresh on EVERY invocation, so
the scratch dir always holds a current access token and never needs to
refresh one. Do not let a long-lived copy refresh independently: OAuth
refresh rotation from two holders can race and invalidate the main
session's credentials. On Linux, credentials live in
~/.claude/.credentials.json and are copied instead (untested path).

Model pinning: the scratch dir carries no model preference, so --pure
without --model serves the CLI-wide default, not your usual session
model. Measurement runs should pin --model explicitly and assert the
served model from the stderr provenance line.

Known flag hazards (verified): --disallowedTools "*" blocks the
StructuredOutput tool itself — the verdict dies in permission_denials.
--system-prompt alone does NOT remove CLAUDE.md/memory; only the
scratch config dir does.
"""
import argparse
import hashlib
import json
import os
import platform
import subprocess
import sys
from pathlib import Path

QUOTA_MARKERS = ("session limit", "resets ", "rate limit")
PURE_DIR = Path.home() / ".claude-judge"
PURE_SYSTEM_PROMPT = (
    "You are a careful, impartial judge. Use only the content given in "
    "the task. Answer only through the structured output.")


def ensure_pure_dir():
    """Scratch config dir + fresh credential mirror; returns call env."""
    PURE_DIR.mkdir(mode=0o700, exist_ok=True)
    state = PURE_DIR / ".claude.json"
    if not state.exists():
        main = json.load(open(Path.home() / ".claude.json"))
        keep = {k: main[k] for k in
                ("oauthAccount", "userID", "hasCompletedOnboarding",
                 "lastOnboardingVersion", "installMethod", "autoUpdates")
                if k in main}
        state.write_text(json.dumps(keep))
    if platform.system() == "Darwin":
        acct = os.environ["USER"]
        secret = subprocess.run(
            ["security", "find-generic-password", "-s",
             "Claude Code-credentials", "-a", acct, "-w"],
            capture_output=True, text=True, check=True).stdout.strip()
        if not secret or "'" in secret:
            raise SystemExit("keychain mirror: unexpected secret shape")
        h = hashlib.sha256(str(PURE_DIR).encode()).hexdigest()[:8]
        r = subprocess.run(
            ["security", "-i"],
            input=(f"add-generic-password -U -s "
                   f"'Claude Code-credentials-{h}' -a '{acct}' "
                   f"-w '{secret}'\n"),
            capture_output=True, text=True)
        if r.returncode:
            raise SystemExit(f"keychain mirror failed: {r.stderr[:200]}")
    else:
        src = Path.home() / ".claude" / ".credentials.json"
        if not src.exists():
            raise SystemExit(f"no credential source at {src}")
        dst = PURE_DIR / ".credentials.json"
        dst.write_text(src.read_text())
        dst.chmod(0o600)
    return {**os.environ, "CLAUDE_CONFIG_DIR": str(PURE_DIR)}


def call(prompt, schema, effort, model, timeout, pure_env=None):
    cmd = ["claude", "-p", "--json-schema", schema,
           "--output-format", "json", "--no-session-persistence",
           "--effort", effort]
    if pure_env is not None:
        cmd += ["--system-prompt", PURE_SYSTEM_PROMPT]
    if model:
        cmd += ["--model", model]
    r = subprocess.run(cmd, input=prompt, text=True, capture_output=True,
                       timeout=timeout, env=pure_env,
                       cwd=str(PURE_DIR) if pure_env is not None else None)
    line = r.stdout.strip().splitlines()[-1] if r.stdout.strip() else ""
    try:
        return json.loads(line)
    except json.JSONDecodeError:
        return {"is_error": True, "result": f"unparseable output: "
                f"{r.stdout[-300:]!r} stderr: {r.stderr[-300:]!r}"}


def main():
    ap = argparse.ArgumentParser(add_help=True)
    ap.add_argument("--schema", required=True,
                    help="JSON Schema, inline or a file path")
    ap.add_argument("--effort", default="low",
                    choices=["low", "medium", "high"])
    ap.add_argument("--model", default=None,
                    help="pin the served model (measurement runs should)")
    ap.add_argument("--retries", type=int, default=1)
    ap.add_argument("--timeout", type=int, default=600)
    ap.add_argument("--pure", action="store_true",
                    help="scratch config dir + minimal system prompt: no "
                         "user rules/memory/hooks reach the judge")
    ap.add_argument("prompt", nargs="?", default="-",
                    help="prompt file, or - for stdin (default)")
    a = ap.parse_args()
    schema = a.schema
    try:
        schema_path = Path(schema)
        if schema_path.is_file():
            schema = schema_path.read_text()
    except OSError:
        pass
    json.loads(schema)  # fail fast on invalid schema
    prompt = (sys.stdin.read() if a.prompt == "-"
              else Path(a.prompt).read_text())

    pure_env = ensure_pure_dir() if a.pure else None
    last = None
    for attempt in range(a.retries + 1):
        d = call(prompt, schema, a.effort, a.model, a.timeout, pure_env)
        out = d.get("structured_output")
        if out is not None and not d.get("is_error"):
            u = d.get("usage", {})
            print(json.dumps({
                # The model that did the work: the CLI now lists a Haiku
                # side-call (session title) first in modelUsage, so "first key"
                # read every Fable turn as haiku (2026-09-10). Max cost is the
                # main turn on every observed shape.
                "model": max(d.get("modelUsage", {}) or {None: None},
                             key=lambda k: (d["modelUsage"][k] or {}).get("costUSD", 0) if k else 0),
                "api_ms": d.get("duration_api_ms"),
                "turns": d.get("num_turns"),
                "out_tokens": u.get("output_tokens"),
                "attempt": attempt + 1}), file=sys.stderr)
            print(json.dumps(out, separators=(",", ":")))
            return 0
        last = str(d.get("result", ""))
        if any(m in last.lower() for m in QUOTA_MARKERS):
            print(f"quota: {last[:200]}", file=sys.stderr)
            return 2
        print(f"attempt {attempt + 1} failed: {last[:200]}", file=sys.stderr)
    print(f"exhausted retries: {last[:300]}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
