//! Rating Performance Core — robust ranking, diagnostics, and planning.
//!
//! Rust port of the Python `rating_engine.py` with the same public API:
//! - Config, AttributeParams, RaterParams, Observation, Edge
//! - CalibrationEvidence, SolveSummary, PlanProposal
//! - RatingEngine, plan_edges_for_rater
//!
//! Implementation notes:
//! - Uses dense `nalgebra::DMatrix` + Cholesky instead of SciPy sparse solvers.
//! - IRLS stopping rule is fixed to avoid the `inf`-always-converges quirk
//!   in the original Python (so you actually get multiple robust iterations).
//! - Gauge pinning and rank / planner logic match the Python semantics.

use std::cmp::Ordering;
use std::collections::{hash_map::DefaultHasher, HashMap};
use std::f64::consts::SQRT_2;
use std::hash::{Hash, Hasher};

use crate::seriate::ontology::ContentId;
use serde::{Deserialize, Serialize};

use nalgebra::linalg::{Cholesky, SymmetricEigen};
use nalgebra::{DMatrix, DVector};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use statrs::function::erf::erf;

/// Maximum items to prevent O(n²) memory exhaustion in dense solver.
/// At 5,000 items, matrix requires ~200 MB; larger scales need sparse solver.
const MAX_ITEMS: usize = 5_000;

/// Maximum candidates in planner to prevent DoS via unbounded iteration.
const MAX_CANDIDATES: usize = 50_000;

/// Maximum reps per observation to prevent ranking manipulation via extreme weights.
const MAX_REPS: f64 = 1000.0;

const ENGINE_SPEC_DOMAIN: &str = "cardinal.engine-spec.v2";

/// When the reduced system is small, compute exact diag(L^-1) via Cholesky solves.
const EXACT_DIAG_MAX_DIM: usize = 256;
const DENSE_SOLVE_MAX_DIM: usize = 1024;

type IrlsHuberSolveResult = (
    Vec<f64>,
    Vec<f64>,
    Vec<f64>,
    Vec<f64>,
    Option<math::LinearSolver>,
    bool,
);

type FuseBucketKey = (usize, usize);
type FuseBucketEntry = (f64, f64, String);
// BTreeMap, deliberately: bucket iteration order feeds edge assembly,
// and HashMap's per-instance random ordering made byte-identical solves
// impossible for identical observation multisets (caught by the judgment
// packet's byte-identity pin at ~30 ulps). Determinism of
// multiset → posterior is a solver-level guarantee for the bulk-ingest
// path; BTreeMap makes it structural.
type FuseBuckets = std::collections::BTreeMap<FuseBucketKey, Vec<FuseBucketEntry>>;

mod diagnostics;
mod engine;
mod math;
mod ranking;
mod types;

pub use diagnostics::{
    compute_hodge_split, spectral_diagnostics, HodgeSplit, LooDiagnostics, SpectralDiagnostics,
};
pub use engine::{plan_edges_for_rater, windowed_candidates, PlannerMode, RatingEngine};
pub use types::{
    AttributeParams, CalibrationEvidence, Config, Edge, EngineSpec, Observation, PlanProposal,
    RaterParams, SolveSummary,
};

pub(crate) use ranking::normal_cdf;
