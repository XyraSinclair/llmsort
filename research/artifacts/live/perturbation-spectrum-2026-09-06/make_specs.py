#!/usr/bin/env python3
"""Build perturbation_spectrum spec files for both pools."""
import hashlib, json

def jitter(text, probe):
    """Port of experiments whitespace_jitter: widen 1+(probe-1)%3 single-space
    gaps to 2-3 spaces; positions from blake-ish hash (we use sha256 — the
    exact gap choice is immaterial, the ledgered prompt is the record)."""
    if probe == 0:
        return text
    b = text.encode()
    gaps = [i for i in range(len(b)) if b[i:i+1] == b' '
            and (i == 0 or b[i-1:i] != b' ') and (i+1 == len(b) or b[i+1:i+2] != b' ')]
    if not gaps:
        return text
    h = hashlib.sha256(f'{text}\x00{probe}'.encode()).digest()
    n_widen = 1 + (probe - 1) % 3
    chosen = sorted({gaps[h[k] % len(gaps)] for k in range(n_widen)})
    out, prev = [], 0
    for k, i in enumerate(chosen):
        width = 2 + (h[8 + k] % 2)
        out.append(text[prev:i]); out.append(' ' * width); prev = i + 1
    out.append(text[prev:])
    return ''.join(out)

def axes(base, paras):
    ax = [{'rung': 'nonce', 'variant': 'base', 'prompt': base}]
    for k in range(1, 7):
        ax.append({'rung': 'jitter', 'variant': f'j{k}', 'prompt': jitter(base, k)})
    for k, p in enumerate(paras, 1):
        ax.append({'rung': 'para', 'variant': f'p{k}', 'prompt': p})
    return ax

# ---- pool 1: lesswrong comments (production pool of jrun_f7dc2335) ----
lw_base = ("Audit for applause-light phrases: words that feel like conclusions "
           "but assert nothing checkable.")
lw_paras = [
    "Check for applause lights: phrasing that sounds like a conclusion while asserting nothing you could verify.",
    "Look for applause-light wording — phrases that feel conclusive but make no checkable claim.",
    "Screen for applause-light language: expressions that carry the feel of a conclusion yet assert nothing testable.",
    "Hunt for applause lights — words that read as if something has been concluded although nothing verifiable is claimed.",
    "Inspect for applause-light phrases, meaning wording that gives the sense of a conclusion but contains no claim that could be checked.",
    "Evaluate for applause-light rhetoric: statements that feel like conclusions while committing to nothing checkable.",
]
entities = []
for line in open('/tmp/pool_lesswrong.jsonl'):
    row = json.loads(line)
    entities.append({'id': row['entity_id'], 'text': row['entity_text']})
assert len(entities) == 12
spec = {'model': 'gemma4-31b', 'base_url': 'http://127.0.0.1:8023/v1',
        'concurrency': 8, 'nonce_draws': 8,
        'entities': entities, 'axes': axes(lw_base, lw_paras)}
json.dump(spec, open('/tmp/spec_lesswrong.json', 'w'), indent=1)

# ---- pool 2: anchor — countries by population (true values recorded) ----
countries = [  # (name, population 2024 est, source UN WPP 2024)
    ('India', 1_450_935_791), ('China', 1_419_321_278), ('United States', 345_426_571),
    ('Indonesia', 283_487_931), ('Brazil', 211_998_573), ('Japan', 123_753_041),
    ('Egypt', 116_538_258), ('Germany', 84_552_242), ('Thailand', 71_668_011),
    ('Australia', 26_713_205), ('Portugal', 10_425_292), ('New Zealand', 5_213_944),
]
anchor_base = "How large is this country's population — how many people live in it?"
anchor_paras = [
    "Population size: the number of people who live in the country.",
    "How many inhabitants does the country have?",
    "Judge by total population — the count of people residing in the country.",
    "Which country is home to more people?",
    "Compare the countries by how many people live in them.",
    "Rank by headcount: the total number of residents of the country.",
]
spec2 = {'model': 'gemma4-31b', 'base_url': 'http://127.0.0.1:8023/v1',
         'concurrency': 8, 'nonce_draws': 8,
         'entities': [{'id': name, 'text': name} for name, _ in countries],
         'axes': axes(anchor_base, anchor_paras)}
json.dump(spec2, open('/tmp/spec_anchor.json', 'w'), indent=1)
json.dump({name: pop for name, pop in countries}, open('/tmp/anchor_truth.json', 'w'), indent=1)
print('specs written; lesswrong axes:', len(spec['axes']), 'anchor axes:', len(spec2['axes']))
print('calls per pool: 66 pairs x 2 orient x (8 nonce + 12 single) =', 66*2*(8+12))
