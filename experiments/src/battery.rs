//! Battery-as-data for the Judge Coherence Benchmark.
//!
//! v1 hardwired the battery as compile-time constants (8 texts, fixed pair
//! arrays) — which capped the benchmark at 194 comparisons and made the
//! public-tier design (entity pools, procedural rotation, dev/held-out
//! splits, ~600–1000 comparisons; `research/notes/PUBLIC_BENCH.md`) unbuildable. This
//! module makes the battery a value: [`BatterySpec`] carries everything the
//! runner needs, [`EntityPool`] is a JSON-loadable pool of meaningful
//! entities, and [`BatterySpec::generate`] deterministically expands a pool
//! into a battery at any scale from a seed. Same seed + pool + scale →
//! byte-identical battery; specs serialize, so every run is replayable even
//! if the generator later changes.
//!
//! [`BatterySpec::v1`] reproduces the original constants exactly — the
//! scripted-pathology suite (`tests/judge_bench.rs`) and all cached v1
//! judgements are untouched.

use serde::{Deserialize, Serialize};

/// The v1 public corpus: eight short texts spanning depth on the primary
/// attribute. Fixed — the v1 benchmark is a standardized instrument, and the
/// consistency dimensions are unfakeable by memorizing the corpus (they
/// constrain *relations between answers*, not answers).
pub const CORPUS: [&str; 8] = [
    "The obstacle is the way.",
    "We suffer more often in imagination than in reality.",
    "No man ever steps in the same river twice.",
    "A journey of a thousand miles begins with a single step.",
    "What gets measured gets managed.",
    "Early to bed and early to rise makes a man healthy, wealthy and wise.",
    "Live, laugh, love.",
    "Monday is the first day of the work week.",
];

/// v1 primary attribute: what the corpus is judged by.
pub const PRIMARY_ATTRIBUTE: &str = "depth of insight about living well";
/// The negation: a coherent judge's scores under it must anti-correlate.
pub const OPPOSITE_ATTRIBUTE: &str =
    "shallowness: the absence of any real insight about living well";
/// A rewording: a coherent judge's scores under it must correlate.
pub const PARAPHRASE_ATTRIBUTE: &str = "how much genuine wisdom about how to live it carries";

/// v1 spin pairs: three with a clear expected direction gap (survival is
/// scoreable) and two genuinely contested (χ is the measurement).
pub const SPIN_CLEAR_PAIRS: [(usize, usize); 3] = [(0, 7), (1, 6), (2, 7)];
pub const SPIN_CONTESTED_PAIRS: [(usize, usize); 2] = [(0, 1), (3, 4)];

/// v1 null texts: judged against themselves.
pub const NULL_INDICES: [usize; 4] = [0, 3, 5, 7];

/// Distinct, contentless passages for measuring slot-A position prior.
pub const NULL_CONTENT_TEXTS: [&str; 2] = [
    "Placeholder passage alpha. It contains no claim, preference, or evaluation.",
    "Placeholder passage beta. It contains no claim, preference, or evaluation.",
];

/// Nuisance perturbations: semantically-null text edits a genuine judge
/// must see through. Part of the axis definition, not battery data — every
/// battery runs the same five.
pub const PERTURBATIONS: [&str; 5] = ["whitespace", "markdown", "bullet", "halo", "container"];

fn default_null_content_texts() -> [String; 2] {
    NULL_CONTENT_TEXTS.map(str::to_string)
}

/// The v1 harmonic block: four texts judged ONLY around a chordless 4-cycle
/// (both orders), disjoint from the main corpus graph. The stride graph's
/// triangles span its whole cycle space (harmonic_dim = 0, pinned in
/// tests/hodge_split.rs), so triad-invisible frustration is unmeasurable
/// there BY CONSTRUCTION; this block has cycle_dim = 1, zero triangles,
/// harmonic_dim = 1 — any non-closure of the loop is pure harmonic
/// energy, the kind no triad audit can ever see.
pub const HARMONIC_BLOCK: [&str; 4] = [
    "Fortune favors the bold.",
    "Look before you leap.",
    "He who hesitates is lost.",
    "Slow and steady wins the race.",
];

/// The chordless cycle over the harmonic block (block-local indices).
pub const HARMONIC_CYCLE: [(usize, usize); 4] = [(0, 1), (1, 2), (2, 3), (0, 3)];

/// v1 pair design over the 8 corpus items: strides 1, 2, and 4 around the
/// ring — 20 pairs, connected and cycle-rich (triangles everywhere), so the
/// curl estimate has support. Generalized by [`ring_stride_pairs`].
#[must_use]
pub fn core_pairs() -> Vec<(usize, usize)> {
    ring_stride_pairs(CORPUS.len(), &[1, 2, 4])
}

/// v1 core pairs that get the perturbation battery (every 3rd pair: 6 of 20).
#[must_use]
pub fn perturb_pairs() -> Vec<(usize, usize)> {
    core_pairs().into_iter().step_by(3).take(6).collect()
}

/// v1 core pairs that get the full Z₂³ orbit transform (6 of 20).
#[must_use]
pub fn orbit_pairs() -> Vec<(usize, usize)> {
    core_pairs()
        .into_iter()
        .skip(1)
        .step_by(3)
        .take(6)
        .collect()
}

/// Ring pairs `(i, (i+s) mod n)` for each stride `s`, normalized and
/// deduplicated in first-encounter order. Connected for stride 1; triangles
/// arise from stride pairs `(s, s, 2s)`, so a doubling stride set is
/// triangle-rich at every scale.
#[must_use]
pub fn ring_stride_pairs(n: usize, strides: &[usize]) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for &stride in strides {
        for i in 0..n {
            let j = (i + stride) % n;
            if i == j {
                continue;
            }
            let (a, b) = if i < j { (i, j) } else { (j, i) };
            if !pairs.contains(&(a, b)) {
                pairs.push((a, b));
            }
        }
    }
    pairs
}

/// Doubling strides `1, 2, 4, …` up to `n/2`: the v1 design generalized.
#[must_use]
pub fn doubling_strides(n: usize) -> Vec<usize> {
    let mut strides = Vec::new();
    let mut k = 1usize;
    while k <= n / 2 {
        strides.push(k);
        k *= 2;
    }
    strides
}

/// Battery generation error.
#[derive(Debug, thiserror::Error)]
pub enum BatteryError {
    #[error("pool too small: {need} items needed ({why}), pool has {have}")]
    PoolTooSmall {
        need: usize,
        have: usize,
        why: &'static str,
    },
    #[error("pool attribute needs at least one paraphrase wording")]
    NoParaphrase,
    #[error("invalid battery: {0}")]
    Invalid(String),
}

/// One entity in a pool. Items are listed in rough DESCENDING prior order
/// of the primary attribute — the ordering drives spin-pair selection
/// (clear pairs span the range, contested pairs are neighbors) and is never
/// shown to the judged model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolItem {
    pub id: String,
    pub text: String,
    /// Optional ground-truth magnitude on a ratio scale (anchors tier).
    /// When every corpus item carries one, the report gains a
    /// magnitude-calibration sidebar (fused log-ratios vs true log-ratios).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truth: Option<f64>,
}

/// The attribute family a pool is judged by.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolAttribute {
    /// The attribute proper.
    pub primary: String,
    /// Its negation: scores must anti-correlate.
    pub opposite: String,
    /// Rewording bank: the generator seeds one per battery (procedural
    /// rotation — wordings are generated per version, not fixed strings).
    pub paraphrases: Vec<String>,
}

/// A pool of meaningful entities from which batteries are generated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityPool {
    pub slug: String,
    pub attribute: PoolAttribute,
    /// In rough descending prior order of `attribute.primary`.
    pub items: Vec<PoolItem>,
    /// Reserved harmonic-block entities (disjoint from `items`); when
    /// absent, the generator reserves the last four unused pool items.
    #[serde(default)]
    pub harmonic: Vec<PoolItem>,
}

/// Scale knobs for battery generation. `for_n` reproduces the v1
/// proportions at any corpus size.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryScale {
    /// Corpus size drawn from the pool.
    pub corpus_n: usize,
    /// Identical-item null pairs.
    pub null_count: usize,
    /// Clear-gap spin pairs (survival scoreable).
    pub spin_clear: usize,
    /// Contested spin pairs (χ measurement).
    pub spin_contested: usize,
    /// Core pairs given the 5-perturbation nuisance battery; `None` → 30%
    /// of core pairs (the v1 share).
    #[serde(default)]
    pub perturb_count: Option<usize>,
    /// Core pairs given the Z₂³ orbit transform; `None` → 30% of core
    /// pairs. Orbit is 8 calls/pair — the dominant cost knob.
    #[serde(default)]
    pub orbit_count: Option<usize>,
}

impl BatteryScale {
    /// v1 proportions at corpus size `n`: null ≈ n/2, spin-clear ≈
    /// max(3, n/4), spin-contested ≈ max(2, n/8), perturb/orbit 30% of
    /// core pairs.
    #[must_use]
    pub fn for_n(n: usize) -> Self {
        Self {
            corpus_n: n,
            null_count: (n / 2).clamp(2, 10),
            spin_clear: (n / 4).max(3),
            spin_contested: (n / 8).max(2),
            perturb_count: None,
            orbit_count: None,
        }
    }
}

/// Everything one benchmark run needs, as a value. Serializable so a
/// generated battery ships with its evidence pack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatterySpec {
    /// Names the battery in reports and packs (e.g. `jcb-v1.2`,
    /// `anchors-country-population-v1/n16/s42`).
    pub slug: String,
    pub corpus: Vec<String>,
    /// Ground-truth magnitudes aligned with `corpus` (anchors tier only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truths: Option<Vec<f64>>,
    pub primary_attribute: String,
    pub opposite_attribute: String,
    pub paraphrase_attribute: String,
    pub core_pairs: Vec<(usize, usize)>,
    pub spin_clear_pairs: Vec<(usize, usize)>,
    pub spin_contested_pairs: Vec<(usize, usize)>,
    pub null_indices: Vec<usize>,
    /// Distinct contentless texts judged in both orders to isolate position
    /// prior from content preference.
    #[serde(default = "default_null_content_texts")]
    pub null_content_texts: [String; 2],
    pub perturb_pairs: Vec<(usize, usize)>,
    pub orbit_pairs: Vec<(usize, usize)>,
    pub harmonic_block: Vec<String>,
    pub harmonic_cycle: Vec<(usize, usize)>,
}

impl BatterySpec {
    /// The original fixed battery — byte-identical to the v1 constants.
    #[must_use]
    pub fn v1() -> Self {
        Self {
            slug: "jcb-v1.2".to_string(),
            corpus: CORPUS.iter().map(|s| s.to_string()).collect(),
            truths: None,
            primary_attribute: PRIMARY_ATTRIBUTE.to_string(),
            opposite_attribute: OPPOSITE_ATTRIBUTE.to_string(),
            paraphrase_attribute: PARAPHRASE_ATTRIBUTE.to_string(),
            core_pairs: core_pairs(),
            spin_clear_pairs: SPIN_CLEAR_PAIRS.to_vec(),
            spin_contested_pairs: SPIN_CONTESTED_PAIRS.to_vec(),
            null_indices: NULL_INDICES.to_vec(),
            null_content_texts: default_null_content_texts(),
            perturb_pairs: perturb_pairs(),
            orbit_pairs: orbit_pairs(),
            harmonic_block: HARMONIC_BLOCK.iter().map(|s| s.to_string()).collect(),
            harmonic_cycle: HARMONIC_CYCLE.to_vec(),
        }
    }

    /// Total provider calls in one run of this battery.
    #[must_use]
    pub fn calls_per_run(&self) -> usize {
        self.core_pairs.len() * 2                       // core, both orders
            + self.core_pairs.len() * 2                 // opposite + paraphrase
            + self.null_indices.len()
            + 2                                         // null-content, both orders
            + self.perturb_pairs.len() * PERTURBATIONS.len()
            + (self.spin_clear_pairs.len() + self.spin_contested_pairs.len()) * 6
            + self.orbit_pairs.len() * 8
            + self.harmonic_cycle.len() * 2
    }

    /// Deterministically generate a battery from a pool: same
    /// `(pool, scale, seed)` → the same battery, forever (splitmix64, no
    /// external RNG). Different seeds rotate the corpus subset, null/spin
    /// positions, and the paraphrase wording — the anti-memorization
    /// mechanism of `research/notes/PUBLIC_BENCH.md`.
    pub fn generate(
        pool: &EntityPool,
        scale: &BatteryScale,
        seed: u64,
    ) -> Result<Self, BatteryError> {
        if pool.attribute.paraphrases.is_empty() {
            return Err(BatteryError::NoParaphrase);
        }
        let n = scale.corpus_n;
        if n < 4 {
            return Err(BatteryError::Invalid(format!("corpus_n {n} < 4")));
        }
        let harmonic_from_pool = pool.harmonic.len() >= 4;
        let need = if harmonic_from_pool { n } else { n + 4 };
        if pool.items.len() < need {
            return Err(BatteryError::PoolTooSmall {
                need,
                have: pool.items.len(),
                why: if harmonic_from_pool {
                    "corpus"
                } else {
                    "corpus + 4-item harmonic block"
                },
            });
        }

        let mut rng = SplitMix64::new(seed ^ fnv1a(pool.slug.as_bytes()));

        // Corpus subset: seeded choice, pool (prior) order preserved.
        let mut idx: Vec<usize> = (0..pool.items.len()).collect();
        rng.shuffle(&mut idx);
        let mut chosen: Vec<usize> = idx[..n].to_vec();
        chosen.sort_unstable();
        let corpus: Vec<String> = chosen.iter().map(|&k| pool.items[k].text.clone()).collect();
        let truths: Option<Vec<f64>> = chosen
            .iter()
            .map(|&k| pool.items[k].truth)
            .collect::<Option<Vec<f64>>>()
            .filter(|t| t.iter().all(|v| *v > 0.0));

        // Harmonic block: reserved pool items, or 4 unused corpus-pool items.
        let harmonic_block: Vec<String> = if harmonic_from_pool {
            let mut hidx: Vec<usize> = (0..pool.harmonic.len()).collect();
            rng.shuffle(&mut hidx);
            hidx[..4]
                .iter()
                .map(|&k| pool.harmonic[k].text.clone())
                .collect()
        } else {
            idx[n..n + 4]
                .iter()
                .map(|&k| pool.items[k].text.clone())
                .collect()
        };

        // Core graph: doubling strides — the v1 construction at any n.
        let core = ring_stride_pairs(n, &doubling_strides(n));

        // Spin: clear pairs span the prior range, contested pairs are
        // prior-neighbors at seeded positions.
        let spin_clear: Vec<(usize, usize)> = (0..scale.spin_clear.min(n / 2))
            .map(|i| (i, n - 1 - i))
            .filter(|&(a, b)| b > a + n / 3)
            .collect();
        let mut starts: Vec<usize> = (0..n - 1).collect();
        rng.shuffle(&mut starts);
        let spin_contested: Vec<(usize, usize)> = starts
            .iter()
            .take(scale.spin_contested.min(n - 1))
            .map(|&j| (j, j + 1))
            .collect();

        // Null indices: seeded distinct.
        let mut all: Vec<usize> = (0..n).collect();
        rng.shuffle(&mut all);
        let mut null_indices: Vec<usize> = all[..scale.null_count.min(n)].to_vec();
        null_indices.sort_unstable();

        // Perturb / orbit shares of the core pairs (v1: every 3rd, 30%).
        let default_share = (core.len() * 3).div_ceil(10);
        let perturb_count = scale.perturb_count.unwrap_or(default_share);
        let orbit_count = scale.orbit_count.unwrap_or(default_share);
        let perturb_pairs: Vec<(usize, usize)> = core
            .iter()
            .copied()
            .step_by(3)
            .take(perturb_count)
            .collect();
        let orbit_pairs: Vec<(usize, usize)> = core
            .iter()
            .copied()
            .skip(1)
            .step_by(3)
            .take(orbit_count)
            .collect();

        // Paraphrase wording: seeded rotation over the bank.
        let paraphrase = pool.attribute.paraphrases
            [(rng.next() as usize) % pool.attribute.paraphrases.len()]
        .clone();

        let spec = Self {
            slug: format!("{}/n{}/s{}", pool.slug, n, seed),
            corpus,
            truths,
            primary_attribute: pool.attribute.primary.clone(),
            opposite_attribute: pool.attribute.opposite.clone(),
            paraphrase_attribute: paraphrase,
            core_pairs: core,
            spin_clear_pairs: spin_clear,
            spin_contested_pairs: spin_contested,
            null_indices,
            null_content_texts: default_null_content_texts(),
            perturb_pairs,
            orbit_pairs,
            harmonic_block,
            harmonic_cycle: HARMONIC_CYCLE.to_vec(),
        };
        spec.validate()?;
        Ok(spec)
    }

    /// Structural invariants every battery must satisfy before spending a
    /// dollar on it. Fails loudly; a battery that cannot support its own
    /// estimators is a bug, not a run.
    pub fn validate(&self) -> Result<(), BatteryError> {
        let n = self.corpus.len();
        let err = |msg: String| Err(BatteryError::Invalid(msg));
        if n < 4 {
            return err(format!("corpus has {n} items; need >= 4"));
        }
        if let Some(truths) = &self.truths {
            if truths.len() != n {
                return err("truths not aligned with corpus".to_string());
            }
            if truths.iter().any(|t| !(t.is_finite() && *t > 0.0)) {
                return err("truths must be finite and > 0 (ratio scale)".to_string());
            }
        }
        for &(i, j) in &self.core_pairs {
            if i >= j || j >= n {
                return err(format!("core pair ({i},{j}) out of order/bounds (n={n})"));
            }
        }
        let mut seen = std::collections::HashSet::new();
        if !self.core_pairs.iter().all(|p| seen.insert(*p)) {
            return err("duplicate core pair".to_string());
        }
        // Connectivity + cycle support of the core graph (union-find).
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(parent: &mut Vec<usize>, x: usize) -> usize {
            if parent[x] != x {
                let root = find(parent, parent[x]);
                parent[x] = root;
            }
            parent[x]
        }
        for &(i, j) in &self.core_pairs {
            let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
            parent[ri] = rj;
        }
        let components = (0..n)
            .map(|x| find(&mut parent, x))
            .collect::<std::collections::HashSet<_>>()
            .len();
        if components != 1 {
            return err(format!(
                "core graph has {components} components; must be connected"
            ));
        }
        let cycle_dim = self.core_pairs.len() + components - n;
        if cycle_dim == 0 {
            return err("core graph is a tree: curl has no support".to_string());
        }
        // Nuisance drift is measured against the same pair's core call.
        for p in &self.perturb_pairs {
            if !self.core_pairs.contains(p) {
                return err(format!(
                    "perturb pair {p:?} not a core pair (no drift baseline)"
                ));
            }
        }
        for &(i, j) in self
            .spin_clear_pairs
            .iter()
            .chain(&self.spin_contested_pairs)
            .chain(&self.orbit_pairs)
        {
            if i == j || i >= n || j >= n {
                return err(format!("spin/orbit pair ({i},{j}) invalid (n={n})"));
            }
        }
        if self.null_indices.iter().any(|&i| i >= n) {
            return err("null index out of bounds".to_string());
        }
        if self.null_content_texts.iter().any(String::is_empty)
            || self.null_content_texts[0] == self.null_content_texts[1]
        {
            return err("null-content texts must be nonempty and distinct".to_string());
        }
        // Harmonic block: a single chordless cycle — every vertex degree 2.
        let h = self.harmonic_block.len();
        if h < 4 {
            return err(format!("harmonic block has {h} items; need >= 4"));
        }
        if self.harmonic_cycle.len() != h {
            return err("harmonic cycle must have exactly one edge per vertex-count".to_string());
        }
        let mut degree = vec![0usize; h];
        for &(i, j) in &self.harmonic_cycle {
            if i == j || i >= h || j >= h {
                return err(format!("harmonic edge ({i},{j}) invalid (h={h})"));
            }
            degree[i] += 1;
            degree[j] += 1;
        }
        if degree.iter().any(|&d| d != 2) {
            return err("harmonic cycle is not a single ring (vertex degree != 2)".to_string());
        }
        Ok(())
    }
}

/// splitmix64: tiny, dependency-free, stable-forever seeded RNG. Batteries
/// must regenerate byte-identically across releases; an external RNG's
/// algorithm is not a contract, this is.
struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Fisher–Yates.
    fn shuffle<T>(&mut self, xs: &mut [T]) {
        for i in (1..xs.len()).rev() {
            let j = (self.next() % (i as u64 + 1)) as usize;
            xs.swap(i, j);
        }
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}
