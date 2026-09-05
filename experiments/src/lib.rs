//! # llmsort-experiments
//!
//! The research side of llmsort: experimental verbs, live batteries, the
//! `cardinald` judgement-run daemon, and instruments whose evidence is not
//! yet in. Nothing here is published or promised; an instrument graduates
//! into the `llmsort` crate only after its evidence pack earns it.
//!
//! Evidence packs, campaign definitions, dated notes, and structured
//! judgements live in this repo's `research/` directory; `PROGRAM.md`
//! at the repo root indexes every method as a rung with its pack.

pub mod anp;
pub mod battery;
pub mod bench;
pub mod canonize;
pub mod ensemble;
pub mod evaluation;
pub mod experiments;
pub mod judgement_run;
pub mod landing;
pub mod openpriors;
pub mod probes;
pub mod slate;
pub mod transitivity;

pub use anp::{anp, AnpAlternative, AnpCriterion, AnpError, AnpOptions, AnpReport};
pub use battery::{
    core_pairs, doubling_strides, orbit_pairs, perturb_pairs, ring_stride_pairs, BatteryError,
    BatteryScale, BatterySpec, EntityPool, PoolAttribute, PoolItem, CORPUS, HARMONIC_BLOCK,
    HARMONIC_CYCLE, NULL_INDICES, OPPOSITE_ATTRIBUTE, PARAPHRASE_ATTRIBUTE, PRIMARY_ATTRIBUTE,
};
pub use bench::{
    render_report as render_bench_report, run_judge_bench, BenchCall, DimensionStat,
    JudgeBenchOptions, JudgeBenchReport,
};
pub use canonize::{
    canonize, planned_sorts, CandidateCanonicality, CanonizeError, CanonizeOptions, CanonizeReport,
};
pub use ensemble::{judge_geometry, JudgeGeometry, JudgePortfolioEntry};
pub use experiments::{
    expand_prompt_experiment_request, AttributePolarity, AttributeVariantSpec,
    PromptExperimentConfig, PromptExperimentError,
};
pub use probes::{
    render_probe_report, run_probe_battery, whitespace_jitter, PairProbes, ProbeBatteryOptions,
    ProbeBatteryReport,
};
pub use slate::{propose_slate, SlateEntry, SlateError, SlateReport};
pub use transitivity::{stochastic_transitivity, TransitivityReport, TriadTest};
