#!/usr/bin/env python3
"""Daily Hacker News technologist_alpha judgement rail.

Fetches the trailing 24h of HN stories (Algolia, points >= MIN_POINTS),
enriches each with its top comments (the technical meat lives there; no
external page scraping), and bulk-judges the day with llmsort's setwise
instrument — Gemma 4 31B, wide concurrency, auto ring rounds — landing a
dated JSON + markdown leaderboard under ~/.local/state/hn-alpha/.

Judge selection, axis wording, and admission evidence:
research/notes/axis-research-2026-07-24/RESULTS-WAVE3.md.

Usage: hn-alpha.py [--hours 24] [--min-points 5] [--concurrency 64]
                   [--model google/gemma-4-31b-it] [--force]
Requires OPENROUTER_API_KEY. Scheduling is host-local (e.g. a launchd
LaunchAgent invoking `xyra-vault run <repo> -- python3 ops/hn-alpha.py`
daily); the job is idempotent per Pacific day, so re-runs are free.
"""
import argparse
import concurrent.futures
import datetime
import json
import os
import re
import subprocess
import sys
import time
import urllib.parse
import urllib.request
from pathlib import Path
from zoneinfo import ZoneInfo

REPO = Path(__file__).resolve().parent.parent
LLMSORT = REPO / "target/release/llmsort"
STATE = Path.home() / ".local/state/hn-alpha"
ALGOLIA = "https://hn.algolia.com/api/v1/search_by_date"
ALGOLIA_REL = "https://hn.algolia.com/api/v1/search"
AXIS_WORDING = json.loads(
    (REPO / "research/notes/axis-research-2026-07-24/wave3/prompts.json").read_text()
)["technologist_alpha"]


def get_json(url: str, tries: int = 4):
    for attempt in range(tries):
        try:
            with urllib.request.urlopen(url, timeout=30) as r:
                return json.load(r)
        except urllib.error.HTTPError as e:
            if attempt == tries - 1:
                raise
            # Algolia rate-limits bursty re-runs with 403; back off hard.
            time.sleep(20 * (attempt + 1) if e.code in (403, 429) else 2 ** attempt)
        except Exception:
            if attempt == tries - 1:
                raise
            time.sleep(2 ** attempt)


def fetch_stories(hours: int, min_points: int):
    since = int(time.time()) - hours * 3600
    stories, page = [], 0
    while True:
        q = urllib.parse.urlencode({
            "tags": "story",
            "numericFilters": f"created_at_i>{since},points>={min_points}",
            "hitsPerPage": 1000,
            "page": page,
        })
        d = get_json(f"{ALGOLIA}?{q}")
        stories += d["hits"]
        page += 1
        if page >= d.get("nbPages", 1):
            return stories


def top_comments(story_id: int, n: int = 3) -> list[str]:
    q = urllib.parse.urlencode({"tags": f"comment,story_{story_id}", "hitsPerPage": n})
    d = get_json(f"{ALGOLIA_REL}?{q}")
    out = []
    for h in d.get("hits", []):
        t = re.sub(r"<[^>]+>", " ", h.get("comment_text") or "")
        t = re.sub(r"\s+", " ", t).strip()
        if t:
            out.append(t)
    return out


def one_line(s: str, limit: int) -> str:
    return re.sub(r"\s+", " ", s).strip()[:limit]


def build_items(stories, workers: int = 12):
    with concurrent.futures.ThreadPoolExecutor(workers) as ex:
        comments = list(ex.map(lambda h: top_comments(h["story_id"]), stories))
    items = []
    for h, cs in zip(stories, comments):
        title = one_line(h.get("title") or "", 200)
        if not title:
            continue
        domain = urllib.parse.urlparse(h.get("url") or "").netloc
        head = f"{title} [{domain or 'self'}, {h.get('points', 0)}pts]"
        body = " | ".join(one_line(c, 260) for c in cs[:3])
        text = f"{head} — comments: {body}" if body else head
        items.append({"story_id": h["story_id"], "title": title,
                      "url": h.get("url") or f"https://news.ycombinator.com/item?id={h['story_id']}",
                      "points": h.get("points", 0), "text": one_line(text, 900)})
    return items


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--hours", type=int, default=24)
    ap.add_argument("--min-points", type=int, default=5)
    ap.add_argument("--concurrency", type=int, default=64)
    ap.add_argument("--model", default="google/gemma-4-31b-it")
    ap.add_argument("--force", action="store_true")
    a = ap.parse_args()

    if not os.environ.get("OPENROUTER_API_KEY"):
        print("OPENROUTER_API_KEY missing", file=sys.stderr)
        return 2
    day = datetime.datetime.now(ZoneInfo("America/Los_Angeles")).strftime("%Y-%m-%d")
    STATE.mkdir(parents=True, exist_ok=True)
    out_json, out_md = STATE / f"{day}.json", STATE / f"{day}.md"
    if out_json.exists() and not a.force:
        print(f"exists: {out_json}")
        return 0

    t0 = time.time()
    stories = fetch_stories(a.hours, a.min_points)
    items = build_items(stories)
    if not items:
        print("no stories in window", file=sys.stderr)
        return 1
    t_fetch = time.time() - t0
    lines = "\n".join(it["text"] for it in items)

    t1 = time.time()
    p = subprocess.run(
        [str(LLMSORT), "sort", "-", "--by", AXIS_WORDING, "--model", a.model,
         "--setwise", "--k", "8", "--concurrency", str(a.concurrency),
         "--format", "json", "--scores"],
        input=lines, capture_output=True, text=True)
    if p.returncode != 0:
        print(p.stderr[-2000:], file=sys.stderr)
        return 1
    t_judge = time.time() - t1
    sorted_out = json.loads(p.stdout)
    by_text = {it["text"]: it for it in items}
    ranked = []
    for row in sorted(sorted_out["items"], key=lambda x: x["rank"]):
        it = by_text.get(row["text"], {})
        ranked.append({**it, "rank": row["rank"], "score": row.get("score")})

    record = {
        "day": day, "axis": "technologist_alpha", "wording": AXIS_WORDING,
        "model": a.model, "n_stories": len(items),
        "fetch_seconds": round(t_fetch, 1), "judge_seconds": round(t_judge, 1),
        "calls": sorted_out.get("calls"), "gauge": sorted_out.get("gauge"),
        "cost_usd": sorted_out.get("cost_nanodollars", 0) / 1e9,
        "stderr_gauge": p.stderr.strip()[-300:],
        "ranking": ranked,
    }
    out_json.write_text(json.dumps(record, indent=1))

    md = [f"# HN technologist_alpha — {day}",
          "",
          f"{len(items)} stories (trailing {a.hours}h, ≥{a.min_points}pts) · "
          f"judge {a.model} · setwise k=8 c={a.concurrency} · "
          f"{record['calls']} calls in {record['judge_seconds']}s · "
          f"${record['cost_usd']:.4f} · flip "
          f"{(record['gauge'] or {}).get('flip_rate', float('nan')):.3f}",
          ""]
    for r in ranked[:40]:
        md.append(f"{r['rank']:3d}. [{r['title']}]({r['url']}) ({r['points']}pts)")
    out_md.write_text("\n".join(md) + "\n")
    print(f"{day}: {len(items)} stories, judged in {t_judge:.1f}s "
          f"(${record['cost_usd']:.4f}, {record['calls']} calls) -> {out_md}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
