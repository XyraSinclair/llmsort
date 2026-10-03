//! Decimal-ledger evidence kernel: credal pushforward of free-form decimal
//! ratio elicitation (research instrument, slug `decimal_ledger_v1`).
//!
//! The instrument elicits `{"higher_ranked": "A"|"B", "ratio": "<D{1,3}.D>"}`
//! at temperature 1 and harvests the token-level stochastic trie across K
//! redraws: every draw returns its own EXACT chosen-token logprob (even below
//! top-k truncation) plus top-k sidebands per position, so redraws mint exact
//! probability atoms — "resampling is peeling" (notes/decimal-pmf-2026-08-10,
//! RESULTS.md census + SHOOTOUT.md ground-truth validation: envelope coverage
//! 1.00 over 4,500 replications, ~47x call efficiency over frequency MC at
//! high enumerated mass).
//!
//! Everything here is pure and deterministic. Coordinates are PRESENTED
//! A-over-B natural log: positive mean = slot A has more (the cardinal
//! convention; note the notes-pack prototype used log10(B/A)).
//!
//! Estimator doctrine (measured, SHOOTOUT.md findings 2 & 4):
//! - point estimate: midpoint-imputation ledger mean when enumerated mass is
//!   high; cross-fit head+residual when low. NEVER same-batch
//!   discover-then-subtract (bias −0.60 measured).
//! - variance: envelope width (uniform imputation) + provider-jitter
//!   bootstrap, both honest per HARVEST.md's three-layer report.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::gateway::TokenLogprob;

/// Declared instrument domain for the elicited ratio.
pub const DOMAIN_LO: f64 = 1.0;
pub const DOMAIN_HI: f64 = 999.9;

/// Instrument-grammar identity for persisted raw draws. The grammar is part
/// of the prompt physics (census finding 1b: the schema is in the model's
/// context and changes beliefs), so replayed estimators must only consume
/// draws minted under the grammar they understand. Bump when the elicited
/// JSON shape, the ratio pattern, or the domain changes.
pub const GRAMMAR_VERSION: &str = "decimal-ratio-v1";

/// Enumerated-mass threshold above which midpoint imputation is the point
/// estimate; below it the cross-fit head+residual estimator takes over
/// (SHOOTOUT.md finding 4).
const ENUM_MASS_POINT_THRESHOLD: f64 = 0.9;

const BOOTSTRAP_REPS: usize = 200;

fn zmax() -> f64 {
    DOMAIN_HI.ln()
}

/// Signed log-ratio of one grammar leaf in presented coordinates, clamped
/// into the declared domain (total and bounded; grammar-legal sub-1.0
/// ratios collapse to 0 exactly as in the notes-pack prototype).
fn zval(dir: char, r: f64) -> f64 {
    let s = if dir == 'A' { 1.0 } else { -1.0 };
    s * r.clamp(DOMAIN_LO, DOMAIN_HI).ln()
}

/// One stochastic node observation from a single draw: the exact
/// chosen-token probability plus top-k sideband probabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeObs {
    pub chosen: (String, f64),
    pub top: Vec<(String, f64)>,
}

/// One parsed draw trajectory through the three stochastic nodes of the
/// instrument grammar: direction, integer token, fraction digit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawTrajectory {
    pub dir: char,
    pub int_tok: String,
    pub frac_tok: String,
    /// Node observations: `[direction, integer, fraction]`.
    pub nodes: [NodeObs; 3],
}

/// The raw material of one ledger judgement, persisted for estimator
/// replay: every parsed draw trajectory (exact chosen-token masses plus
/// sidebands at each stochastic node) and the grammar version that minted
/// them. `analyze(&record.draws)` recomputes the judgement's evidence from
/// these observations alone — no provider call, no cache — so estimator
/// upgrades can rescore historical traces offline. Replayers must check
/// `grammar_version == GRAMMAR_VERSION` before analyzing; draws from a
/// different grammar are a different instrument, not older data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerDrawsRecord {
    pub grammar_version: String,
    pub draws: Vec<DrawTrajectory>,
}

impl DrawTrajectory {
    /// The draw's own leaf ratio.
    pub fn ratio(&self) -> Option<f64> {
        format!("{}.{}", self.int_tok, self.frac_tok).parse().ok()
    }

    fn leaf(&self) -> (char, String, String) {
        (self.dir, self.int_tok.clone(), self.frac_tok.clone())
    }
}

/// Extract the instrument trajectory from a response's output logprobs.
///
/// Grammar-adjacency is enforced strictly: the integer must be ONE bare
/// digit-token, immediately followed by the '.' token, immediately
/// followed by a single bare digit-token fraction. Multi-token integers
/// (non-o200k digit grouping) are rejected rather than silently
/// mis-binned — a sharper guard than the notes-pack prototype, which
/// could misread a split integer's first token as the whole integer.
///
/// Tokens are matched RAW (no whitespace trimming), mirroring the
/// prototype's `token[:1].isdigit()` semantics: a leading-space token
/// like ` 12` is a prose-position number (e.g. "about 12.5 times" in a
/// non-JSON-mode preamble), not the string-literal grammar position, and
/// accepting it would mint atoms from prose conditionals
/// (port-fidelity review, 2026-08-11).
pub fn extract_trajectory(tokens: &[TokenLogprob]) -> Option<DrawTrajectory> {
    fn bare_digits(tok: &str) -> bool {
        !tok.is_empty() && tok.chars().all(|c| c.is_ascii_digit())
    }
    let mut seen = String::new();
    let mut dir_i = None;
    let mut int_i = None;
    let mut dot_i = None;
    let mut frac_i = None;
    for (i, t) in tokens.iter().enumerate() {
        // Anchor on the full JSON key: a "reason" field whose prose
        // contains the word "higher" followed by a bare A/B token would
        // otherwise mint a sign-flipped direction atom (falsifier
        // FRAGILE-1, 2026-08-11). Token concatenation makes the key
        // substring robust to any tokenizer split of "higher_ranked".
        let prev_contains_higher = seen.contains("higher_ranked");
        let prev_contains_ratio = seen.contains("ratio");
        seen.push_str(&t.token);
        if dir_i.is_none() && (t.token == "A" || t.token == "B") && prev_contains_higher {
            dir_i = Some(i);
        }
        if dir_i.is_some() && int_i.is_none() && prev_contains_ratio && bare_digits(&t.token) {
            int_i = Some(i);
            continue;
        }
        if let Some(ii) = int_i {
            if dot_i.is_none() && i > ii {
                if t.token == "." && i == ii + 1 {
                    dot_i = Some(i);
                    continue;
                }
                if bare_digits(&t.token) {
                    // integer split across tokens: reject the draw
                    return None;
                }
            }
        }
        if let Some(di) = dot_i {
            if frac_i.is_none() && i == di + 1 {
                if t.token.len() == 1 && bare_digits(&t.token) {
                    frac_i = Some(i);
                } else {
                    return None;
                }
            }
        }
    }
    let (dir_i, int_i, frac_i) = (dir_i?, int_i?, frac_i?);
    // Probability sanitation (falsifier BUG-3, 2026-08-11): providers can
    // emit logprob > 0 (mass > 1) and, in principle, non-finite values.
    // NaN defeats every downstream `<= 0.0` guard and +inf poisons the
    // moments; a non-finite chosen probability rejects the draw, and all
    // masses are clamped into [0, 1] at intake (-inf → 0 is already sane).
    for &i in &[dir_i, int_i, frac_i] {
        let p = tokens[i].logprob.exp();
        if p.is_nan() || p == f64::INFINITY {
            return None;
        }
    }
    let node = |i: usize| NodeObs {
        chosen: (
            tokens[i].token.clone(),
            tokens[i].logprob.exp().clamp(0.0, 1.0),
        ),
        top: tokens[i]
            .top_alternatives
            .iter()
            .filter(|alt| alt.logprob.exp().is_finite())
            .map(|alt| (alt.token.clone(), alt.logprob.exp().clamp(0.0, 1.0)))
            .collect(),
    };
    let dir = if tokens[dir_i].token == "A" { 'A' } else { 'B' };
    Some(DrawTrajectory {
        dir,
        int_tok: tokens[int_i].token.clone(),
        frac_tok: tokens[frac_i].token.clone(),
        nodes: [node(dir_i), node(int_i), node(frac_i)],
    })
}

/// Per-token mass observations at one trie node, across draws.
#[derive(Debug, Clone, Default)]
pub struct NodeStats {
    pub obs: BTreeMap<String, Vec<f64>>,
}

impl NodeStats {
    fn add(&mut self, tok: &str, p: f64) {
        self.obs.entry(tok.to_string()).or_default().push(p);
    }

    fn masses(&self) -> BTreeMap<String, f64> {
        self.obs
            .iter()
            .map(|(t, v)| (t.clone(), v.iter().sum::<f64>() / v.len() as f64))
            .collect()
    }
}

/// The three-level stochastic trie: `[]` (direction), `[dir]` (integer),
/// `[dir, int]` (fraction). BTreeMap keys keep iteration deterministic.
pub type Trie = BTreeMap<Vec<String>, NodeStats>;

/// Accumulate draw trajectories into the trie. Top-k sidebands are folded
/// in alongside chosen-token observations, RAW: distinct token strings
/// (` 12` vs `12`) stay distinct obs vectors. Merging them would let a
/// near-zero whitespace variant dilute a real token's drift-averaged mass;
/// kept raw, the variant simply fails the ledger's digit/letter filters
/// and routes conservatively into residual cells, matching the prototype
/// (port-fidelity review, 2026-08-11).
pub fn accumulate(draws: &[DrawTrajectory]) -> Trie {
    let mut trie = Trie::new();
    for d in draws {
        let keys: [Vec<String>; 3] = [
            vec![],
            vec![d.dir.to_string()],
            vec![d.dir.to_string(), d.int_tok.clone()],
        ];
        for (key, node_obs) in keys.iter().zip(d.nodes.iter()) {
            let ns = trie.entry(key.clone()).or_default();
            let (ref tok, p) = node_obs.chosen;
            ns.add(tok, p);
            for (tok, p) in &node_obs.top {
                ns.add(tok, *p);
            }
        }
    }
    trie
}

/// Z-range of an unresolved cell (direction known, integer optional),
/// intersected with the declared domain. Presented coordinates.
fn zrange_cell(dir: char, int_tok: Option<&str>) -> (f64, f64) {
    let s = if dir == 'A' { 1.0 } else { -1.0 };
    let (lo, hi) = match int_tok.and_then(|t| t.parse::<f64>().ok()) {
        None => (0.0, zmax()),
        Some(i) => {
            let r_lo = i.max(DOMAIN_LO);
            let r_hi = (i + 0.95).min(DOMAIN_HI);
            if r_hi < r_lo {
                // Cell wholly below the domain (integer 0): every leaf 0.x
                // clamps to z = 0 under the validated convention, so the
                // cell's Z-range is exactly {0} — not full-domain slack
                // (unified with certify's atom handling, 2026-08-11).
                (0.0, 0.0)
            } else {
                (r_lo.ln(), r_hi.ln())
            }
        }
    };
    let (a, b) = (s * lo, s * hi);
    (a.min(b), a.max(b))
}

/// An unresolved-mass cell with its adversarial Z-range.
#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub mass: f64,
    pub z_lo: f64,
    pub z_hi: f64,
}

type MassFn<'a> = &'a dyn Fn(&NodeStats) -> BTreeMap<String, f64>;

/// Exact probability atoms keyed by grammar leaf (direction, integer
/// token, fraction digit).
type Atoms = BTreeMap<(char, String, String), f64>;

/// Build the exact-atom ledger from the trie: atoms carry products of
/// measured node masses; unattributed mass becomes cells with Z-ranges.
fn ledger_with(trie: &Trie, mass_fn: MassFn) -> (Atoms, Vec<Cell>) {
    let mut atoms = BTreeMap::new();
    let mut cells = Vec::new();
    let empty = NodeStats::default();
    let dir_m = mass_fn(trie.get(&vec![]).unwrap_or(&empty));
    let mut dir_known = 0.0;
    for (d_tok, &p_d) in &dir_m {
        let dir = match d_tok.as_str() {
            "A" => 'A',
            "B" => 'B',
            _ => continue,
        };
        if p_d <= 0.0 {
            continue;
        }
        dir_known += p_d;
        let Some(int_node) = trie.get(&vec![d_tok.clone()]) else {
            let (z_lo, z_hi) = zrange_cell(dir, None);
            cells.push(Cell {
                mass: p_d,
                z_lo,
                z_hi,
            });
            continue;
        };
        let int_m = mass_fn(int_node);
        let mut acc_int = 0.0;
        for (i_tok, &p_i) in &int_m {
            if p_i <= 0.0 || i_tok.is_empty() || !i_tok.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            acc_int += p_i;
            let Some(frac_node) = trie.get(&vec![d_tok.clone(), i_tok.clone()]) else {
                let (z_lo, z_hi) = zrange_cell(dir, Some(i_tok));
                cells.push(Cell {
                    mass: p_d * p_i,
                    z_lo,
                    z_hi,
                });
                continue;
            };
            let frac_m = mass_fn(frac_node);
            let mut acc_frac = 0.0;
            for (f_tok, &p_f) in &frac_m {
                if p_f <= 0.0 || f_tok.len() != 1 || !f_tok.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                acc_frac += p_f;
                atoms.insert((dir, i_tok.clone(), f_tok.clone()), p_d * p_i * p_f);
            }
            let resid_f = (1.0 - acc_frac).max(0.0);
            if resid_f > 1e-9 {
                let (z_lo, z_hi) = zrange_cell(dir, Some(i_tok));
                cells.push(Cell {
                    mass: p_d * p_i * resid_f,
                    z_lo,
                    z_hi,
                });
            }
        }
        let resid_i = (1.0 - acc_int).max(0.0);
        if resid_i > 1e-9 {
            let (z_lo, z_hi) = zrange_cell(dir, None);
            cells.push(Cell {
                mass: p_d * resid_i,
                z_lo,
                z_hi,
            });
        }
    }
    let resid_d = (1.0 - dir_known).max(0.0);
    if resid_d > 1e-9 {
        cells.push(Cell {
            mass: resid_d,
            z_lo: -zmax(),
            z_hi: zmax(),
        });
    }
    (atoms, cells)
}

/// The credal certificate over E\[Z\].
#[derive(Debug, Clone, Copy)]
pub struct Certificate {
    pub enumerated_mass: f64,
    pub cell_mass: f64,
    pub out_of_domain_mass: f64,
    /// Mass the drift-averaged ledger cannot attribute; soundness demands
    /// it WIDEN the envelope as full-domain slack, never silently vanish.
    pub conservation_gap: f64,
    pub e_lo: f64,
    pub e_hi: f64,
    pub e_mid: f64,
}

impl Certificate {
    pub fn width(&self) -> f64 {
        self.e_hi - self.e_lo
    }
}

fn certify(atoms: &BTreeMap<(char, String, String), f64>, cells: &[Cell]) -> Certificate {
    let head: f64 = atoms.values().sum();
    let mut e_head = 0.0;
    let mut out_of_domain = 0.0;
    for ((dir, i_tok, f_tok), &p) in atoms {
        let Ok(r) = format!("{i_tok}.{f_tok}").parse::<f64>() else {
            out_of_domain += p;
            continue;
        };
        // Sub-domain ratios (integer token "0") follow the validated clamp
        // convention everywhere: zval maps them to exactly 0, so their z is
        // KNOWN and they belong in the head, not in full-domain slack
        // (coherence review 2026-08-11: certify previously paid ±zmax
        // envelope width for atoms whose value is exact under the same
        // file's own zval convention).
        e_head += p * zval(*dir, r);
    }
    let cell_mass: f64 = cells.iter().map(|c| c.mass).sum();
    let gap = (1.0 - head - cell_mass).abs();
    let cell_lo: f64 = cells.iter().map(|c| c.mass * c.z_lo).sum();
    let cell_hi: f64 = cells.iter().map(|c| c.mass * c.z_hi).sum();
    let cell_mid: f64 = cells.iter().map(|c| c.mass * (c.z_lo + c.z_hi) / 2.0).sum();
    Certificate {
        enumerated_mass: head,
        cell_mass,
        out_of_domain_mass: out_of_domain,
        conservation_gap: gap,
        e_lo: e_head + cell_lo + (out_of_domain + gap) * (-zmax()),
        e_hi: e_head + cell_hi + (out_of_domain + gap) * zmax(),
        e_mid: e_head + cell_mid,
    }
}

/// Cross-fit head+residual estimator (SHOOTOUT.md finding 2): the first
/// half of the draws fixes the exact-mass head; the second half estimates
/// the residual conditional mean from draws OUTSIDE the head. Same-batch
/// discover-then-subtract is structurally biased (measured −0.60) and is
/// deliberately not implemented.
fn crossfit_point(draws: &[DrawTrajectory]) -> Option<f64> {
    if draws.len() < 2 {
        return None;
    }
    let (head_half, est_half) = draws.split_at(draws.len() / 2);
    let trie = accumulate(head_half);
    let (atoms, _) = ledger_with(&trie, &|ns| ns.masses());
    let q_c: f64 = atoms.values().sum();
    let mut e_head = 0.0;
    for ((dir, i_tok, f_tok), &p) in &atoms {
        if let Ok(r) = format!("{i_tok}.{f_tok}").parse::<f64>() {
            e_head += p * zval(*dir, r);
        }
    }
    let outside: Vec<f64> = est_half
        .iter()
        .filter(|d| !atoms.contains_key(&d.leaf()))
        .filter_map(|d| d.ratio().map(|r| zval(d.dir, r)))
        .collect();
    // No out-of-head draw in the estimation half: impute 0 (the center of
    // the symmetric Z-range); the contribution is bounded by (1-q_c)*zmax.
    let resid_mean = if outside.is_empty() {
        0.0
    } else {
        outside.iter().sum::<f64>() / outside.len() as f64
    };
    Some(e_head + (1.0 - q_c).max(0.0) * resid_mean)
}

/// Deterministic xorshift64* for the provider-jitter bootstrap.
struct XorShift(u64);

impl XorShift {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn choose<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[(self.next_u64() % v.len() as u64) as usize]
    }
}

/// Provider-noise band: resample each token's mass from its observed
/// values, rebuild the ledger, recompute the midpoint E\[Z\]; the std of
/// those replicates is the jitter component of the reported variance
/// (HARVEST.md finding 3: continuous 1-2% logit jitter, not backend
/// bimodality).
fn bootstrap_std(trie: &Trie) -> f64 {
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    let mut vals = Vec::with_capacity(BOOTSTRAP_REPS);
    for _ in 0..BOOTSTRAP_REPS {
        // Resample one observed mass per token (the RNG cannot ride inside
        // a Fn mass_fn, so build a single-observation trie per replicate).
        let mut tmp = Trie::new();
        for (key, ns) in trie {
            let mut resampled = NodeStats::default();
            for (tok, obs) in &ns.obs {
                resampled.obs.insert(tok.clone(), vec![*rng.choose(obs)]);
            }
            tmp.insert(key.clone(), resampled);
        }
        let (atoms, cells) = ledger_with(&tmp, &|ns| ns.masses());
        vals.push(certify(&atoms, &cells).e_mid);
    }
    let n = vals.len() as f64;
    let mean = vals.iter().sum::<f64>() / n;
    (vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt()
}

/// The fused outcome of a K-draw decimal-ledger judgement.
#[derive(Debug, Clone, Copy)]
pub struct LedgerOutcome {
    /// Point estimate of the signed log-ratio, presented coordinates.
    pub mean: f64,
    /// Honest variance: envelope width (uniform imputation, width²/12)
    /// plus the provider-jitter bootstrap component.
    pub var: f64,
    pub enumerated_mass: f64,
    pub envelope_width: f64,
    pub e_lo: f64,
    pub e_hi: f64,
    pub conservation_gap: f64,
    /// P(direction = A) from the direction-node masses (for the point
    /// judgement's confidence surface).
    pub p_dir_a: f64,
}

/// Fuse parsed draw trajectories into a single evidence outcome.
/// Returns `None` when no trajectory parsed.
///
/// Deliberate hybrid in the low-enumeration regime: the MEAN switches to
/// the cross-fit estimator (SHOOTOUT.md finding 4) while the VARIANCE
/// stays envelope-shaped (`width²/12` + a bootstrap of the midpoint
/// statistic). The envelope covers truth regardless of which point
/// estimator is reported (coverage 1.00 in the ground-truth shootout).
/// To account for the hybrid honestly, the squared DISAGREEMENT between
/// the two point estimators is folded into the variance whenever both are
/// computable (coherence review 2026-08-11): it is exactly the
/// estimator-choice variance the threshold switch would otherwise leave
/// unreported, it removes the discontinuity's unaccounted component, and
/// it absorbs part of the cross-fit half-split sensitivity.
pub fn analyze(draws: &[DrawTrajectory]) -> Option<LedgerOutcome> {
    if draws.is_empty() {
        return None;
    }
    let trie = accumulate(draws);
    let (atoms, cells) = ledger_with(&trie, &|ns| ns.masses());
    let cert = certify(&atoms, &cells);
    // The point estimate must live inside its own credal envelope: the
    // cross-fit residual mean assigns unresolved mass to the est-half
    // sample mean, which tail draws can push past the cells' bounded
    // Z-ranges (falsifier BUG-2, 2026-08-11 — a certified-incompatible
    // observation would otherwise reach the solver). The certificate is
    // the sound object; clamp the point into it.
    let crossfit = crossfit_point(draws).map(|cf| cf.clamp(cert.e_lo, cert.e_hi));
    let mean = if cert.enumerated_mass >= ENUM_MASS_POINT_THRESHOLD {
        cert.e_mid
    } else {
        crossfit.unwrap_or(cert.e_mid)
    };
    let disagreement = crossfit.map_or(0.0, |cf| cert.e_mid - cf);
    let boot = bootstrap_std(&trie);
    let width = cert.width();
    let var = width * width / 12.0 + boot * boot + disagreement * disagreement;
    let empty = NodeStats::default();
    let dir_m = trie.get(&vec![]).unwrap_or(&empty).masses();
    let p_a = dir_m.get("A").copied().unwrap_or(0.0);
    let p_b = dir_m.get("B").copied().unwrap_or(0.0);
    let p_dir_a = if p_a + p_b > 0.0 {
        p_a / (p_a + p_b)
    } else {
        0.5
    };
    Some(LedgerOutcome {
        mean,
        var,
        enumerated_mass: cert.enumerated_mass,
        envelope_width: width,
        e_lo: cert.e_lo,
        e_hi: cert.e_hi,
        conservation_gap: cert.conservation_gap,
        p_dir_a,
    })
}
