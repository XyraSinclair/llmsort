# Principles: staying ahead of slop

How this repo keeps shipping profound work in its canonical flavor —
structured, provenanced, invariance-tested LLM judgment. Each principle
earned its place by catching something real; the evidence is cited. Read
before any substantial work session.

## 1. Refutability is the product

Ship every claim as something that can lose: two-sided pins with dated
history, planted pathologies that must be caught, controlled experiments
aimed at our own hypotheses. A session that never proved itself wrong
wasn't testing anything.
(Evidence: ladder-curl experiment killed our own quantization hypothesis;
nano's polarity headline died on retest; the Cesàro convergence criterion
was unreachable and the unit pins caught it; the byte-sum hash judge
failed to avalanche and the attack had to be made competent.)

## 2. Validate the instrument on scripted pathologies before live claims

A probe that has never caught a planted fake measures nothing. Every
benchmark axis and probe ships with the adversary it kills, in the test
suite: oracle, constant, position-biased, sycophant (linear AND
threshold), cyclic, avalanche-hash, inversion-blind, sign-broken-channel.
New axis ⇒ new scripted judge that dies by exactly it.

## 3. No claim without its denominator and noise class

n = 1 runs are instrument demonstrations, labeled as such — never
model properties. Relation axes (polarity, paraphrase) carry ~4× the
run-to-run noise of direction axes (flip, spin, curl) at current corpus
size: know an axis's noise floor before narrating its value. Test–retest
is a standing cost of every leaderboard version, not a one-off. (Evidence:
mean |ΔJUDGE| 0.022 but polarity swung ±0.4 on the same model.)

## 4. Mathematics is the register

Report the measured functional property — slope, R², odd/even
decomposition, curl fraction, probability-mass flow — never the
personality ("stiffens", "follows the asker"). Physics vocabulary is
admitted only where the math is literal (Hodge curl, gauge freedom,
linear response), never as decoration. If a finding cannot be stated as a
number with units and a denominator, it is not ready. This binds hardest
on our own instruments.

## 5. Probes graduate into estimators

The maturity ladder for every invariance: measured violation → nuisance
parameter fitted jointly (template gains) → equivariant-by-construction
elicitation (counterbalancing; orbit sampling next). Quote uncertainty
experimentalist-style — statistical and systematic side by side in native
units, never silently pooled. A belief whose systematics dominate is not
improved by more sampling; it names the transformation to fix.

## 6. Evidence or it didn't happen; errata on top, never rewrites

Every published number recomputable from committed raw judgements (JSONL
+ replayable cache in dated `research/artifacts/live/` packs). When a claim dies,
append the erratum to the original pack and correct downstream surfaces
the same day—the evidence stands, and the correction stands on it.

## 7. Adversary before audience

Red-team every design before it ships: enumerate the exploits per axis,
pair each with a countermeasure or list it as OPEN in the doc. (Evidence:
reciprocity double-counting and refusal-laundering were fixed pre-launch
because an adversary pass demanded it; the open-attack list IS the v2
spec.) Also red-team the fix: a countermeasure that never faced its
bypass is untested.

## 8. Eat the dogfood at every level

Choose judges by our own coherence benchmark; prioritize our roadmap with
our own ANP; audit our own sessions with our own probes' discipline.
Self-application is the demo and the fastest bug source; it is never the
only validation — each map version scores at least one reference the
stack didn't produce and we didn't write.

## 9. Composition over surface; pay the bloat tax on every add

New capability must be composition of the seven primitives (entity,
attribute, presentation, judgement, evidence, weight, posterior). If it
needs a parallel implementation, fix the primitives instead. When you add
surface, consolidate surface: dedupe shared math to one canonical fn
(the reflection rule was pasted three times before it was one), prune a
flag, or mark something experimental.

## 10. The transformation group is the roadmap

Before "what feature next," ask "which transformation should not matter,
and is it instrumented?" New work = a new row of the invariance table, a
deeper treatment of an existing row (probe → estimator), or scale for a
row whose noise floor blocks claims. The ✗ rows are ranked future work;
an instrument no claim has needed in two versions is a retirement
candidate. No instrument or axis is admitted without a pre-registered
rubric and a retirement condition named at birth (adopted 2026-08-10
after the 30-day test measured zero retirements in 131+ commits).

## 11. Prompt bytes are physics

The judge sees rendered bytes, not intentions. Templates HTML-escape `"`
and `'` (both shipped garbled framings before tests caught them); rtk
rewrites shell output; caches key on exact strings. Verify what the model
actually receives (`--show-prompt`, od) before interpreting what it
answers. Wording hygiene is measurement hygiene — and fixed wordings are
memorizable, so probe wordings rotate procedurally at scale.

## 12. Everything is denominated

Nanodollars, calls, tokens, and noise floors on every result; worst-case
pricing before every run; cost of the credibility number itself
(test–retest ≈ $0.50/board) budgeted per version. Sub-$0.10 costs get
one significant figure — four decimal places on a penny is ritual, not
accounting. A measurement whose
cost is unknown cannot be scaled; a benchmark whose reproducibility is
unpaid-for cannot be trusted.
