"""Step 0: two small hard cohorts and the Fable reference prompts.
Cohorts: 24 Manifund applications (asks stripped) and 24 arXiv abstracts, seed 19. Attributes: the 12
elaborated high-dimensional attributes (research/batteries/highdim_attributes_elaborated.txt).
Reference design: per cohort 2 replicas; a replica is 2 Fable reads of 6 attributes each, with its own item
shuffle, neutral labels, and attribute split (halves / alternating), so replica agreement is a reliability."""
import json, os, random
HERE = os.path.dirname(os.path.abspath(__file__)); R = f"{HERE}/../../.."
rng = random.Random(19)
attrs = [l.strip() for l in open(f"{R}/batteries/highdim_attributes_elaborated.txt") if l.strip()]
attrs = [{"name": a.split(":")[0].strip(), "text": a} for a in attrs]
mf = [x for x in json.load(open(f"{R}/data/manifund/items/full_noask.json")) if 2500 <= len(x["text"]) <= 6000]
bench = json.load(open(f"{R}/batteries/multi-criteria-bench/arxiv.json"))
cohorts = {"manifund": rng.sample(mf, 24), "arxiv": rng.sample(bench, 24)}
json.dump(attrs, open(f"{HERE}/attributes.json", "w"), indent=1)
for name, items in cohorts.items():
    json.dump(items, open(f"{HERE}/{name}.json", "w"), indent=1)
    for rep in (0, 1):
        order = list(range(24)); random.Random(100 + rep).shuffle(order)
        split = [list(range(0, 6)), list(range(6, 12))] if rep == 0 else [list(range(0, 12, 2))[::-1], list(range(1, 12, 2))[::-1]]
        for half, ai in enumerate(split):
            labels = {f"T{k + 1:02d}": items[i]["id"] for k, i in enumerate(order)}
            body = "\n\n".join(f"=== {lab} ===\n{items[i]['text']}" for lab, i in zip(labels, order))
            al = "\n".join(f"- {attrs[a]['text']}" for a in ai)
            out = f"<pack>/ref/{name}-r{rep}-h{half}.json"  # the launch message tells the judge where <pack> is
            prompt = f"""You are a careful judge building a reference dataset. Below are 24 texts and 6 attributes. For EACH attribute, judge all 24 texts on a RATIO scale by magnitude estimation: the typical (median) text is 1.0; a text with twice as much of the attribute is 2.0, half as much 0.5; use the range the texts deserve (0.1 to 10 is available) and avoid ties unless two texts are truly indistinguishable. Judge each attribute on its own definition, independently of the other attributes and of how good the text is overall; the definitions say what to reward and what not to. Work attribute by attribute: compare texts against each other, settle the order, then set magnitudes.

Do not use any tool except Read (this file) and Write (the output). No scripts, no subagents, no web.
Write ONLY a JSON object to {out} of the form {{"<attribute name>": {{"T01": 1.3, ..., "T24": 0.6}}, ...}} using exactly these attribute names: {json.dumps([attrs[a]['name'] for a in ai])}. Then reply with the single word done.

ATTRIBUTES
{al}

TEXTS
{body}
"""
            open(f"{HERE}/ref/{name}-r{rep}-h{half}.prompt.txt", "w").write(prompt)
            json.dump(labels, open(f"{HERE}/ref/{name}-r{rep}-h{half}.labels.json", "w"))
print({k: sum(len(i["text"]) for i in v) // 4 for k, v in cohorts.items()}, "tokens per read")
