"""Fetch truth-bearing cohorts from Wikidata: python3 cohorts.py [name ...]  -> <name>.csv (label,value) + cohorts.json entry.

Each cohort is ~200 well-known entities (sitelink count as the fame filter) with one numeric property; the reference is
log(value) (or the raw year), so every sort has a fact behind it and a slope. lab.py reads any cohort listed in cohorts.json.
"""
import csv, json, os, random, sys, urllib.parse, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
Q = {
    "cities": ("wd:Q515", "wdt:P1082", 100000, 40, "Population: how many people live in the city proper (latest estimate).", "city", "log"),
    "mountains": ("wd:Q8502", "wdt:P2044", 1000, 25, "Elevation: the height of the summit above sea level, in metres.", "mountain", "log"),
    "rivers": ("wd:Q4022", "wdt:P2043", 100, 30, "Length: the length of the river from source to mouth, in kilometres.", "river", "log"),
    "companies": ("wd:Q4830453", "wdt:P2139", 1e8, 25, "Revenue: the company's most recent annual revenue, in US dollars.", "company", "log"),
    "films": ("wd:Q11424", "wdt:P2142", 1e6, 40, "Box office: the film's worldwide theatrical gross, in US dollars.", "film", "log"),
    "novels": ("wd:Q8261", "wdt:P577", 1500, 30, "Publication year: the year the novel was first published (later is higher).", "novel", "year"),
    "people": ("wd:Q5", "wdt:P569", 1500, 120, "Birth year: the year the person was born (later is higher).", "person", "year"),
    "elements": ("wd:Q11344", "wdt:P1086", 0, 0, "Atomic number: the number of protons in the nucleus.", "chemical element", "raw"),
}


def fetch(name):
    cls, prop, lo, links, attr, noun, scale = Q[name]
    val = f"YEAR(?v)" if scale == "year" else "?v"
    q = f"""SELECT ?item ?itemLabel ?links (MAX({val}) AS ?value) WHERE {{
  ?item wdt:P31 {cls}; {prop} ?v; wikibase:sitelinks ?links. FILTER({val} > {lo}) FILTER(?links >= {links})
  SERVICE wikibase:label {{ bd:serviceParam wikibase:language "en". }}
}} GROUP BY ?item ?itemLabel ?links ORDER BY DESC(?links) LIMIT 1500"""
    req = urllib.request.Request("https://query.wikidata.org/sparql?" + urllib.parse.urlencode({"query": q, "format": "json"}), headers={"User-Agent": "llmsort-sortlab/1 (research)"})
    rows = json.load(urllib.request.urlopen(req, timeout=180))["results"]["bindings"]
    rows = [(r["itemLabel"]["value"], float(r["value"]["value"])) for r in rows if not r["itemLabel"]["value"].startswith("Q")]
    seen = set(); rows = [r for r in rows if not (r[0] in seen or seen.add(r[0]))]
    rng = random.Random(5); rows = rows if len(rows) <= 200 else rng.sample(rows[:800], 200)
    with open(f"{HERE}/{name}.csv", "w", newline="") as f:
        w = csv.writer(f); w.writerow(["label", "value"]); w.writerows(rows)
    man = json.load(open(f"{HERE}/cohorts.json")) if os.path.exists(f"{HERE}/cohorts.json") else {}
    man[name] = {"attr": attr, "noun": noun, "scale": scale, "n": len(rows)}
    json.dump(man, open(f"{HERE}/cohorts.json", "w"), indent=1)
    print(f"{name}: {len(rows)} rows, value range {min(v for _, v in rows):g}–{max(v for _, v in rows):g}", flush=True)


for name in (sys.argv[1:] or list(Q)):
    try: fetch(name)
    except Exception as e: print(f"{name}: FAILED {e}", flush=True)
