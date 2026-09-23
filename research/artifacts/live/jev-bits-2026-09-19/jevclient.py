"""Thin client for TypeSafe Jev (POST /v1/systemone) with a banked JSONL trace.

Every response is appended to the trace keyed by the sha256 of the request body, so a rerun
replays from disk and a killed run loses nothing. Key comes from the environment:
  xyra-vault run ~/x/jev/typed-judgment -- python3 <script>
"""
import hashlib, json, os, threading, time, urllib.error, urllib.request
from concurrent.futures import ThreadPoolExecutor

URL = "https://api.typesafe.ai/v1/systemone"
_lock = threading.Lock()


class JevError(Exception):
    pass


def _key(body: bytes, salt: str) -> str:
    return hashlib.sha256(body + salt.encode()).hexdigest()


def load_trace(path):
    seen = {}
    if os.path.exists(path):
        for line in open(path):
            r = json.loads(line)
            seen[r["key"]] = r
    return seen


def call(state, questions, trace_path, seen, salt="", tag=None):
    """questions: {id: {type, instructions, criteria?}}. salt distinguishes deliberate repeats."""
    body = json.dumps({"state": state, "model": "jev-latest", "questions": questions}, sort_keys=True).encode()
    k = _key(body, salt)
    if k in seen:
        return seen[k]
    api_key = os.environ["TYPESAFE_API_KEY"]
    for attempt in range(7):
        req = urllib.request.Request(URL, data=body, method="POST",
                                     headers={"Authorization": f"Bearer {api_key}", "Content-Type": "application/json", "User-Agent": "llmsort-jevclient/1"})
        t0 = time.time()
        try:
            with urllib.request.urlopen(req, timeout=120) as resp:
                d = json.load(resp)
            break
        except urllib.error.HTTPError as e:
            msg = e.read()[:400]
            if e.code in (429, 500, 502, 503, 504) and attempt < 6:
                time.sleep(1.5 * 2 ** attempt)
                continue
            raise JevError(f"{tag}: HTTP {e.code} {msg!r}")
        except (urllib.error.URLError, TimeoutError) as e:
            if attempt < 6:
                time.sleep(1.5 * 2 ** attempt)
                continue
            raise JevError(f"{tag}: {e!r}")
    rec = {"key": k, "tag": tag, "latency_ms": round((time.time() - t0) * 1000, 1), "attempts": attempt + 1,
           "usage": d.get("usage", {}), "answers": {q: _slim(a) for q, a in d["answers"].items()}}
    with _lock:
        with open(trace_path, "a") as f:
            f.write(json.dumps(rec) + "\n")
        seen[k] = rec
    return rec


def _slim(a):
    a = dict(a)
    a.pop("legend", None)
    return a


def call_many(reqs, trace_path, workers=16):
    """reqs: [{state, questions, tag, salt?}] -> records in order."""
    seen = load_trace(trace_path)
    with ThreadPoolExecutor(workers) as ex:
        return list(ex.map(lambda r: call(r["state"], r["questions"], trace_path, seen, r.get("salt", ""), r.get("tag")), reqs))


def pmf(ans, n):
    return [ans["probabilities"][str(i)] for i in range(n)]
