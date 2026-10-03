# North-star ontology (LOCKED 2026-08-12)

Status: LOCKED by the operator 2026-08-12 ("sure, yolo"). Canonized in
`README.md` § "What we are building"; naming map executed the
same day (crate/repo renamed to ratiometer at 0.12.0). This note is the
decision record.

**One-liner:** the instrument that turns an LLM's felt sense into
calibrated measurement. Sorting is the demo; measurement is the product.

**Five nouns:**

1. **Attribute** — any nameable dimension, vibes included.
2. **Magnitude** — the latent positive quantity each entity has of it.
   Only ratios are observable; absolute scale is gauge freedom.
3. **Instrument** — any elicitation trick emitting evidence about
   magnitude ratios (ratio letter, ordinal, decimal ledger, k-wise, …).
   Instruments never touch the solver directly.
4. **Evidence** — the single currency: (E[log-ratio], honest variance).
   Why the instrument zoo stays commensurable.
5. **Scaling** — the output. Lexical fill: a RANKING is ordinal, a
   SCALING is cardinal — a ranking is a scaling with the spacing deleted.

**The missing word for a cardinal-spaced list item: a READING** —
magnitude ± uncertainty on the shared log-ratio scale. Rank says
position; a reading says position, distance, and confidence. Units for
presentation: doublings (or dB); the engine already lives in log-space.

**Rhetorical anchor:** star magnitudes — ordinal rank buckets ("first
magnitude stars") cardinalized by Pogson 1856 via a fixed ratio step
(2.512×). Ordinal rank → fixed-ratio cardinal scale is exactly this
project's move, per attribute. The ratio ladder is Pogson's step,
generalized.

**Why cardinal:** gaps are where decisions live; ratios compose along
paths (sparse coverage + free triangle consistency checks); magnitudes
transfer across lists — readings accumulate into a web of priors
(the OpenPriors endgame) instead of every sort starting from zero.

**Stack:** attribute → magnitude → instruments → evidence → scaling of
readings. LLM-sorter is the storefront; the ratiometer is the machine.

## Naming map (locked + executed 2026-08-12)

Four layers wanting different KINDS of names:

1. **Vocabulary — common nouns, never brand:** scaling, reading,
   magnitude, doubling. The ranking-vs-scaling distinction should
   propagate unattributed; jargon would quarantine it.
2. **Machine (crate/repo) — instrument noun:** **ratiometer**
   (crates.io-free, self-describing) over **pogson** (also free; the
   story-name — Pogson cardinalized ordinal star magnitudes with a fixed
   ratio step in 1856 — but opaque on first contact). Pogson parked as a
   future informal UNIT name (a ladder step / doubling, as bel→decibel).
3. **Storefront — a sentence, not a noun:** "Readings, not rankings."
   pairwiseratio.org headline; states product + leaderboard critique.
4. **OpenPriors — unchanged;** the ontology (readings → web of priors)
   is the name's justification.

Anti-recommendation: instruments stay versioned slugs
(`decimal_ledger_v1`), never brandable names — research-grade surface
must be free to die.
