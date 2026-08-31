#![forbid(unsafe_code)]

use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand, ValueEnum};
use llmsort::cache::SqlitePairwiseCache;
use llmsort::gateway::{NoopUsageSink, ProviderGateway};
use llmsort::rerank::model_policy::ModelPolicy;
use llmsort::rerank::report::validate_report_inputs;
use llmsort::rerank::{
    build_report, load_policy_from_path, render_report_markdown, validate_multi_rerank_request,
    JsonlTraceSink, MultiRerankRequest, MultiRerankResponse, PolicyRegistry, RerankReportOptions,
    RerankRunOptions, TraceSink,
};
use llmsort::Attribution;
use llmsort_experiments::{
    expand_prompt_experiment_request, AttributeVariantSpec, PromptExperimentConfig,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PairwiseModeArg {
    Ratio,
    Ordinal,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ReportFormatArg {
    Md,
    Markdown,
    Json,
}

impl From<PairwiseModeArg> for llmsort_experiments::evaluation::SyntheticPairwiseMode {
    fn from(mode: PairwiseModeArg) -> Self {
        match mode {
            PairwiseModeArg::Ratio => Self::Ratio,
            PairwiseModeArg::Ordinal => Self::Ordinal,
        }
    }
}

#[derive(Parser)]
#[command(
    name = "cardinal",
    version,
    about = "Canonical pairwise ratio CLI",
    after_help = "The stability-promised verbs are `sort` and `judge` (plus the judgment-packet \
format they emit). Verbs marked (research) are honest, provenanced instruments \
that are free to change shape without notice (AGENTS.md: canonical vs \
research-grade surface)."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SortFormatArg {
    Text,
    Json,
    Jsonl,
    Csv,
}

#[derive(Subcommand)]
enum Commands {
    /// Sort a list of items by a natural-language criterion
    ///
    /// Reads newline-delimited items (or a JSON array) from FILE or stdin and
    /// prints them sorted best-first. Requires OPENROUTER_API_KEY unless
    /// --cache-only is set and the cache already holds every judgement.
    ///
    /// Example: cardinal sort examples/sort-demo.txt --by "usefulness as advice"
    Sort {
        /// Input file; '-' or omitted reads stdin
        file: Option<PathBuf>,
        /// Criterion to sort by, e.g. "clarity of explanation"
        #[arg(long)]
        by: String,
        /// Model slug (OpenRouter), e.g. anthropic/claude-sonnet-4.6
        #[arg(long)]
        model: Option<String>,
        /// Built-in model policy name (see `cardinal policy list`)
        #[arg(long)]
        policy: Option<String>,
        /// Model policy JSON file
        #[arg(long)]
        policy_config: Option<PathBuf>,
        /// Maximum pairwise comparisons to spend
        #[arg(long)]
        budget: Option<usize>,
        /// Certify only the top K items (default: whole list)
        #[arg(long)]
        top_k: Option<usize>,
        /// Output format
        #[arg(long, value_enum, default_value = "text")]
        format: SortFormatArg,
        /// In text mode, prefix each line with `mean±std<TAB>`
        #[arg(long)]
        scores: bool,
        /// Worst first instead of best first
        #[arg(long)]
        reverse: bool,
        /// Also judge the OPPOSITE of the criterion (`lack of <criterion>`),
        /// fold it in with weight -1, and report cross-side consistency
        #[arg(long)]
        two_sided: bool,
        /// Alternate phrasing of the criterion; judged as an extra attribute
        /// and reported as a paraphrase-consistency probe (repeatable)
        #[arg(long)]
        also_by: Vec<String>,
        /// Ask each planned pair in one random order only, instead of the
        /// default both-orders counterbalancing (halves cost, loses the
        /// position-bias diagnostic)
        #[arg(long)]
        no_counterbalance: bool,
        /// Prompt template: canonical_v2 (default), canonical_bucket_v1, or
        /// ratio_letter_v1 (single-token PMF evidence via answer logprobs;
        /// degrades loudly to sampled mode where providers hide them)
        #[arg(long)]
        template: Option<String>,
        /// First expand the criterion into a precise judging rubric with one
        /// LLM call, print it to stderr, then sort by the rubric
        #[arg(long)]
        elaborate: bool,
        /// Stop spending exploration comparisons on items whose probability
        /// of reaching the top-k drops below this (requires --top-k intent;
        /// pruned count is reported in the run summary)
        #[arg(long)]
        prune_below: Option<f64>,
        /// RNG seed for reproducible planning
        #[arg(long)]
        seed: Option<u64>,
        /// Judgements in flight at once (default 8). Lower it for
        /// rate-limited rails: a subscription CLI rail that 429s under a burst
        /// backs off for minutes, so 8-wide bursts cost more wall-clock than
        /// a 2-wide steady stream
        #[arg(long)]
        concurrency: Option<usize>,
        /// Serve judgements from cache only; error on any cache miss
        #[arg(long)]
        cache_only: bool,
        /// Do not read or write the pairwise cache
        #[arg(long)]
        no_cache: bool,
        /// SQLite cache path (default: shared user cache)
        #[arg(long)]
        cache: Option<PathBuf>,
        /// Write a JSONL trace of every comparison
        #[arg(long)]
        trace: Option<PathBuf>,
        /// Suppress the run summary on stderr
        #[arg(long)]
        quiet: bool,
        /// Print the worst-case comparison count and dollar cost, then exit
        /// without touching the network or cache
        #[arg(long)]
        estimate: bool,
    },
    /// (research) AHP: weigh attributes against a goal via pairwise comparisons
    ///
    /// Attributes are entities too: each is judged pairwise on "importance
    /// for the goal", and the solver's log-latents are the log priority
    /// vector — softmax gives normalized ratio-scale weights (the
    /// least-squares analog of Saaty's eigenvector). Feed the weights back
    /// into multi-attribute reranking.
    Weigh {
        /// The goal the attributes serve
        #[arg(long)]
        goal: String,
        /// Attribute as name=description (repeatable; at least 2 unless --propose)
        #[arg(long = "attribute")]
        attributes: Vec<String>,
        /// Automated AHP: ask the model to propose N considerations for the
        /// goal, then weigh them (merges with any --attribute entries)
        #[arg(long)]
        propose: Option<usize>,
        /// Model slug (OpenRouter)
        #[arg(long)]
        model: Option<String>,
        /// Prompt template (canonical_v2, ratio_letter_v1, ...)
        #[arg(long)]
        template: Option<String>,
        /// Maximum pairwise comparisons
        #[arg(long)]
        budget: Option<usize>,
        /// RNG seed
        #[arg(long, default_value_t = 7)]
        seed: u64,
        /// SQLite cache path
        #[arg(long)]
        cache: Option<PathBuf>,
        /// Emit weights as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// (research) Find the attributes under which one item stands out from its peers
    ///
    /// The propagation primitive: given a set and a focal item, propose (or
    /// supply) candidate attributes, MEASURE all of them over the whole set
    /// with the counterbalanced pairwise machinery, and report where the
    /// focal item actually lands per attribute — percentile and z-score,
    /// best direction first. Proposals are hypotheses; the profile is the
    /// evidence.
    Distinguish {
        /// Input file; '-' or omitted reads stdin (one item per line, or a
        /// JSON array of strings / {id, text} objects)
        input: Option<PathBuf>,
        /// The focal item: 1-based line number, item id, or exact text
        #[arg(long)]
        focus: String,
        /// Candidate attribute to measure (repeatable)
        #[arg(long = "by")]
        by: Vec<String>,
        /// Ask the model to propose N distinguishing attributes for the
        /// focal item (default 5 when no --by is given)
        #[arg(long)]
        propose: Option<usize>,
        /// Model slug (OpenRouter)
        #[arg(long)]
        model: Option<String>,
        /// Maximum pairwise comparisons across all attributes
        #[arg(long)]
        budget: Option<usize>,
        /// RNG seed
        #[arg(long, default_value_t = 7)]
        seed: u64,
        /// SQLite cache path
        #[arg(long)]
        cache: Option<PathBuf>,
        /// Emit the full profile as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// (research) Slate: item + voice note in, stakeholder-backed attributes out
    ///
    /// The intake front door: name the stakeholders whose judgment of this
    /// item matters (or let the model propose them), have each propose
    /// attributes blind to the others, merge near-duplicates. Breadth —
    /// how many unlike perspectives back an attribute — is the proxy for
    /// "alive in social reality". The slate is hypotheses, priced in
    /// pennies; measure with canonize/sort/distinguish, or invert with
    /// explain when a believed ranking comes first.
    Slate {
        /// The item (literal text, or @path)
        item: String,
        /// Your voice-note ramble about why it's interesting (or @path)
        #[arg(long)]
        note: Option<String>,
        /// Stakeholders, comma-separated (omit to have the model name them)
        #[arg(long)]
        stakeholders: Option<String>,
        /// How many stakeholders to propose when none are given
        #[arg(long, default_value_t = 4)]
        stakeholder_count: usize,
        /// Attributes per stakeholder
        #[arg(long, default_value_t = 4)]
        per: usize,
        /// Model slug (OpenRouter)
        #[arg(long)]
        model: Option<String>,
        /// Emit the full report as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// (research) Canonize an attribute: converge on a communication primitive
    ///
    /// An attribute is canonical when it induces the SAME cardinal latent
    /// in different minds. This measures exactly that: seed wording (plus
    /// LLM-proposed refinements) judged over the entities by every judge
    /// model given; candidates ranked by transmissibility (mean cross-judge
    /// Spearman of the latent vectors), with per-judge signal and
    /// redundancy against already-accepted attributes. The merge protocol
    /// for the canonical-attribute loop: measured, never asserted.
    Canonize {
        /// Entities file; '-' or omitted reads stdin
        input: Option<PathBuf>,
        /// The seed attribute wording
        #[arg(long)]
        by: String,
        /// Ask the model to propose N alternative wordings/refinements
        #[arg(long, default_value_t = 3)]
        propose: usize,
        /// Judge models, comma-separated (at least 2)
        #[arg(long)]
        judges: String,
        /// Already-accepted canonical attribute (repeatable): candidates
        /// are scored for redundancy against these
        #[arg(long = "accepted")]
        accepted: Vec<String>,
        /// TOTAL comparison budget across all sorts (accepted + candidates
        /// × judges), divided evenly; omit to leave each sort on its own
        /// default budget
        #[arg(long)]
        budget: Option<usize>,
        /// RNG seed
        #[arg(long, default_value_t = 7)]
        seed: u64,
        /// SQLite cache path
        #[arg(long)]
        cache: Option<PathBuf>,
        /// Emit the full report as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// (research) Analytic Network Process: goal, criteria, alternatives — with feedback
    ///
    /// AHP generalized to a network. Every supermatrix edge is a solved
    /// pairwise measurement: criteria weighed against the goal, criteria
    /// weighed for their contribution to EACH OTHER (inner dependence),
    /// alternatives measured per criterion, and alternatives feeding back
    /// into criteria weights via their per-criterion z-scores. Priorities
    /// are the Cesàro limit of the influence walk from the goal. The
    /// headline diagnostic: limiting vs direct criteria weights — the network
    /// correction in probability mass.
    Anp {
        /// Alternatives file; '-' or omitted reads stdin (one per line, or
        /// JSON array of strings / {id, text})
        input: Option<PathBuf>,
        /// The goal
        #[arg(long)]
        goal: String,
        /// Criterion as name=description (repeatable)
        #[arg(long = "attribute")]
        attributes: Vec<String>,
        /// Ask the model to propose N criteria for the goal (merges with
        /// --attribute)
        #[arg(long)]
        propose: Option<usize>,
        /// Share of a criterion's influence flowing to other criteria
        /// (inner dependence) vs alternatives, in [0,1)
        #[arg(long, default_value_t = 0.4)]
        alpha: f64,
        /// Model slug (OpenRouter)
        #[arg(long)]
        model: Option<String>,
        /// Comparison budget for the alternatives rerank
        #[arg(long)]
        budget: Option<usize>,
        /// RNG seed
        #[arg(long, default_value_t = 7)]
        seed: u64,
        /// SQLite cache path
        #[arg(long)]
        cache: Option<PathBuf>,
        /// Emit the full report (supermatrix included) as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// (research) Judge Coherence Benchmark: score models on judgement consistency
    ///
    /// No ground-truth labels: the benchmark measures internal consistency
    /// under meaning-preserving transformations (order swap, reciprocal
    /// antisymmetry, cyclic frustration, framing spin, polarity reversal,
    /// paraphrase stability, null calibration) plus a signal axis so a
    /// Whitespace-jitter probe battery: K quasi-independent draws per pair
    ///
    /// Probes the same structured judgement K times, each probe inserting
    /// subtly different amounts of spaces into the attribute prompt at
    /// seed-chosen points before the elicitation. Each probe is a distinct
    /// cache key, so draws accumulate instead of colliding; draws pool via
    /// DerSimonian-Laird with a heterogeneity floor. Reports per-pair
    /// dispersion, sign stability, duplicate rate, and sigma_w2/sigma_b2.
    Probe {
        /// Entities file, one per line
        file: PathBuf,
        /// Attribute to judge by
        #[arg(long)]
        by: String,
        /// Model slug (OpenRouter)
        #[arg(long)]
        model: String,
        /// Probes per pair (probe 0 is the unjittered prompt)
        #[arg(long, default_value_t = 8)]
        probes: u32,
        /// Prompt template slug
        #[arg(long, default_value = "canonical_v2")]
        template: String,
        /// Concurrent comparisons
        #[arg(long, default_value_t = 6)]
        concurrency: usize,
        /// Cap the number of ring pairs (default: n, the full stride-1 ring)
        #[arg(long)]
        pairs: Option<usize>,
        /// Write the full JSON report to this path
        #[arg(long)]
        out: Option<PathBuf>,
        /// Do not read or write the pairwise cache
        #[arg(long)]
        no_cache: bool,
        /// SQLite cache path (default: shared user cache)
        #[arg(long)]
        cache: Option<PathBuf>,
    },
    /// constant judge cannot hide in perfect consistency. Headline score =
    /// signal × coherence. Default battery: the fixed v1.2 corpus, 202
    /// comparisons per model; scale up with --pool (entity-pool JSON →
    /// procedurally generated battery, deterministic in --battery-seed).
    Bench {
        /// Model slug(s), comma-separated
        #[arg(long)]
        models: String,
        /// Prompt template slug
        #[arg(long, default_value = "canonical_v2")]
        template: String,
        /// Concurrent comparisons per model
        #[arg(long, default_value_t = 6)]
        concurrency: usize,
        /// Entity-pool JSON (research/data/pools/): generate the battery
        /// from it instead of the fixed v1.2 corpus
        #[arg(long)]
        pool: Option<PathBuf>,
        /// Corpus size drawn from the pool (default: the whole pool)
        #[arg(long)]
        scale_n: Option<usize>,
        /// Battery generation seed: rotates corpus subset, spin/null
        /// positions, and the paraphrase wording (anti-memorization)
        #[arg(long, default_value_t = 0)]
        battery_seed: u64,
        /// Run an exact battery-spec JSON (overrides --pool)
        #[arg(long)]
        battery: Option<PathBuf>,
        /// Write the resolved battery spec JSON here (replayability)
        #[arg(long)]
        battery_out: Option<PathBuf>,
        /// Write full per-model reports (raw calls included) to this JSONL path
        #[arg(long)]
        out: Option<PathBuf>,
        /// Emit the leaderboard as JSON on stdout
        #[arg(long)]
        json: bool,
        /// Do not read or write the pairwise cache
        #[arg(long)]
        no_cache: bool,
        /// SQLite cache path (default: shared user cache)
        #[arg(long)]
        cache: Option<PathBuf>,
    },
    /// (research) Measure a model's pure elicitation artifacts with null pairs
    ///
    /// Presents byte-identical text in both slots of the ratio-letter
    /// instrument: a perfect judge answers parity, so ANY directional
    /// probability mass is artifact — position prior plus letter prior.
    /// Reports the artifact split and the mean absolute directional bias
    /// in nats, per model. Costs pennies; run it before trusting a judge.
    Calibrate {
        /// Model slug(s), comma-separated
        #[arg(long)]
        models: String,
        /// Number of distinct null texts to probe per model
        #[arg(long, default_value_t = 6)]
        nulls: u8,
        /// Write per-model reports to this JSONL path
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// One pairwise judgement between two items, fully transparent
    ///
    /// The lowest-level primitive: see exactly what the judge is asked
    /// (--show-prompt) and exactly what it answered. Items are literal text
    /// or @path to read a file.
    Judge {
        /// First item (literal text, or @path)
        item_a: String,
        /// Second item (literal text, or @path)
        item_b: String,
        /// Criterion to judge by
        #[arg(long)]
        by: String,
        /// Model slug (OpenRouter)
        #[arg(long)]
        model: Option<String>,
        /// Prompt template slug
        #[arg(long, default_value = "canonical_v2")]
        template: String,
        /// Print the fully rendered system + user prompt to stderr first
        #[arg(long)]
        show_prompt: bool,
        /// Structured JSON output on stdout
        #[arg(long)]
        json: bool,
        /// Do not read or write the pairwise cache
        #[arg(long)]
        no_cache: bool,
        /// SQLite cache path (default: shared user cache)
        #[arg(long)]
        cache: Option<PathBuf>,
        /// Susceptibility probe: judge under neutral, pro-first, and
        /// pro-second requester framings (each in both presentation orders,
        /// 6 comparisons) and report whether the belief survives the spin
        #[arg(long)]
        spin: bool,
        /// Sweep framing intensity from -3 to +3 (14 comparisons) and fit
        /// the response line: chi as a slope plus a linearity R² — separates
        /// a genuinely rigid judge from a threshold sycophant. Implies the
        /// full sweep instead of the 6-call --spin probe.
        #[arg(long)]
        sweep: bool,
        /// Orbit transform: measure the judgment under the full Z₂³ group
        /// (order × polarity × wording, 8 comparisons), pull back through
        /// the known equivariances, and report the character decomposition
        /// — belief = the invariant coefficient, every bias a named
        /// orthogonal coefficient, Parseval as the energy budget
        #[arg(long)]
        orbit: bool,
        /// Repeat the judgement N times varying only a suffix nonce
        /// (cache-friendly: the long prefix stays byte-identical, so
        /// provider prompt caching bills it at the cached rate) and report
        /// the mean, the spread sigma_w — the within-pair
        /// context-sensitivity noise the DL floor consumes — and the
        /// provider's cached-token count
        #[arg(long)]
        draws: Option<usize>,
        /// Sampling temperature for --draws (default 0: spread = pure
        /// context sensitivity, not sampling noise)
        #[arg(long, default_value_t = 0.0)]
        temperature: f32,
        /// Wording-invariance probe: ask the same question as "times more",
        /// "what fraction", and "which has LESS" (6 comparisons) — a
        /// coherent ratio judge must recover the same signed log-ratio
        /// through all three; disagreement separates inversion failure
        /// from numerical framing bias
        #[arg(long)]
        wordings: bool,
        /// Consortium verdict: judge models, comma-separated (≥ 2). Each
        /// judge measures the full Z₂³ orbit (8 comparisons); complete
        /// orbits become judgment packets and the belief is computed by
        /// FUSING them — one number, an explicit error budget (within-judge
        /// orbit bias + cross-judge spread), and portable evidence
        #[arg(long)]
        consortium: Option<String>,
        /// Write one judgment packet JSON per usable judge to this
        /// directory (with --consortium)
        #[arg(long)]
        packets_out: Option<PathBuf>,
    },
    /// (research) Expand a terse criterion into a precise judging rubric (one LLM call)
    ///
    /// Prints only the rubric to stdout, so it composes:
    ///   cardinal sort list.txt --by "$(cardinal elaborate --by impact)"
    Elaborate {
        /// The terse criterion to expand
        #[arg(long)]
        by: String,
        /// Model slug (OpenRouter)
        #[arg(long)]
        model: Option<String>,
    },
    /// (research) Explain an existing ranking: which attributes reconstruct it?
    ///
    /// FILE (or stdin) holds items in YOUR believed order, best first.
    /// Each --candidate attribute is measured with pairwise judgements and
    /// scored on how well it — alone and in weighted combination —
    /// reconstructs your order.
    Explain {
        /// Input file in believed order, best first; '-' or omitted reads stdin
        file: Option<PathBuf>,
        /// Candidate attribute (repeatable)
        #[arg(long)]
        candidate: Vec<String>,
        /// Ask an LLM to propose this many additional candidate attributes
        #[arg(long)]
        propose: Option<usize>,
        /// Model slug (OpenRouter)
        #[arg(long)]
        model: Option<String>,
        /// Total comparison budget across all candidates
        #[arg(long)]
        budget: Option<usize>,
        /// Structured JSON output on stdout
        #[arg(long)]
        format_json: bool,
        /// Do not read or write the pairwise cache
        #[arg(long)]
        no_cache: bool,
        /// SQLite cache path (default: shared user cache)
        #[arg(long)]
        cache: Option<PathBuf>,
        /// RNG seed for reproducible planning
        #[arg(long)]
        seed: Option<u64>,
    },
    /// (research) Export SQLite cache to JSONL
    CacheExport {
        #[arg(long)]
        db: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
    },
    /// (research) Prune SQLite cache by age and/or size
    CachePrune {
        #[arg(long)]
        db: Option<PathBuf>,
        #[arg(long)]
        max_age_days: Option<u64>,
        #[arg(long)]
        max_rows: Option<usize>,
    },
    /// (research) List or load model policies
    Policy {
        #[command(subcommand)]
        command: PolicyCommands,
    },
    /// (research) Run the synthetic pairwise-ratio evaluation suite
    Eval {
        #[arg(long)]
        case: Option<String>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        curve_csv: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "ratio")]
        mode: PairwiseModeArg,
    },
    /// (research) Run the synthetic Likert baseline evaluation suite
    EvalLikert {
        #[arg(long)]
        case: Option<String>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        curve_csv: Option<PathBuf>,
        #[arg(long, default_value_t = 10, value_parser = parse_likert_levels)]
        levels: usize,
        #[arg(long, default_value_t = 1.0, value_parser = parse_positive_finite_f64)]
        budget_multiplier: f64,
    },
    /// (research) Compare cardinal pairwise evaluation against the Likert baseline
    EvalCompare {
        #[arg(long)]
        case: Option<String>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 10, value_parser = parse_likert_levels)]
        levels: usize,
        #[arg(long, default_value_t = 1.0, value_parser = parse_positive_finite_f64)]
        budget_multiplier: f64,
        #[arg(long, value_enum, default_value = "ratio")]
        mode: PairwiseModeArg,
    },
    /// (research) Generate a report from a request + response JSON
    Report {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        response: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, value_enum, default_value = "md")]
        format: ReportFormatArg,
        #[arg(long, default_value_t = 10, value_parser = parse_report_top_n)]
        top_n: usize,
        #[arg(long)]
        include_infeasible: bool,
        #[arg(long)]
        no_attr_scores: bool,
        #[arg(long)]
        rng_seed: Option<u64>,
        #[arg(long)]
        policy: Option<String>,
        #[arg(long)]
        cache_only: bool,
    },
    /// (research) Expand one request across prompt templates and positive/negative attribute variants
    ExperimentExpand {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long = "prompt-template")]
        prompt_template_slugs: Vec<String>,
        #[arg(long)]
        include_negative: bool,
        #[arg(long = "variant-json")]
        variant_json: Vec<PathBuf>,
    },
    /// (research) Validate a multi-rerank request JSON without touching the network or cache
    Validate {
        #[arg(long)]
        request: PathBuf,
    },
    /// (research) Run a rerank from JSON input
    Rerank {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        cache: Option<PathBuf>,
        #[arg(long)]
        lock_cache: bool,
        #[arg(long)]
        cache_only: bool,
        #[arg(long)]
        policy: Option<String>,
        #[arg(long)]
        policy_config: Option<PathBuf>,
        #[arg(long)]
        rng_seed: Option<u64>,
        #[arg(long)]
        report: Option<PathBuf>,
        #[arg(long)]
        trace: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum PolicyCommands {
    List,
    Load {
        #[arg(long)]
        config: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Sort {
            file,
            by,
            model,
            policy,
            policy_config,
            budget,
            top_k,
            format,
            scores,
            reverse,
            two_sided,
            also_by,
            no_counterbalance,
            template,
            elaborate,
            prune_below,
            seed,
            concurrency,
            cache_only,
            no_cache,
            cache,
            trace,
            quiet,
            estimate,
        } => {
            if cache_only && no_cache {
                return Err("--cache-only and --no-cache are mutually exclusive".into());
            }
            let raw = read_sort_input(file.as_deref())?;
            let documents = parse_sort_items(&raw)?;
            if documents.is_empty() {
                return Err("no items to sort: input is empty".into());
            }

            if estimate {
                let opts = llmsort::rerank::SortOptions {
                    model: model.clone(),
                    comparison_budget: budget,
                    top_k,
                    counterbalance: !no_counterbalance,
                    two_sided,
                    also_by: also_by.clone(),
                    prune_p_topk_below: prune_below,
                    prompt_template_slug: template.clone(),
                    ..Default::default()
                };
                let simple = llmsort::rerank::sort::sort_request(documents.clone(), &by, &opts);
                let multi = llmsort::rerank::simple::to_multi_request(&simple);
                let charge = llmsort::rerank::estimate_max_rerank_charge(&multi);
                println!(
                    "worst case: {} comparisons · ~{} input + {} output tokens each · provider max ${:.4}",
                    charge.comparison_budget,
                    charge.input_tokens_per_comparison,
                    charge.output_tokens_per_comparison,
                    charge.provider_cost_max_nanodollars as f64 / 1e9,
                );
                eprintln!(
                    "estimate only — no network, no cache; actual runs stop earlier on certified top-k or cache hits"
                );
                return Ok(());
            }

            let gateway = provider_gateway(cache_only)?;

            let cache_store = if no_cache {
                None
            } else {
                let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
                Some(SqlitePairwiseCache::new(cache_path)?)
            };
            let policy_obj = load_policy(policy, policy_config)?;

            let (trace_sink, trace_worker) = if let Some(path) = trace {
                let (sink, worker) = JsonlTraceSink::new(path)?;
                (Some(sink), Some(worker))
            } else {
                (None, None)
            };
            let trace_ref = trace_sink.as_ref().map(|sink| sink as &dyn TraceSink);

            let gateway = Arc::new(gateway);
            let mut execution = llmsort::rerank::RerankExecution::new(
                gateway.clone(),
                Attribution::new("cardinal::sort"),
            )
            .run_options(RerankRunOptions {
                rng_seed: seed,
                cache_only,
            });
            if let Some(store) = cache_store.as_ref() {
                execution = execution.cache(store);
            }
            if let Some(policy) = policy_obj {
                execution = execution.model_policy(policy);
            }
            if let Some(trace) = trace_ref {
                execution = execution.trace(trace);
            }

            let opts = llmsort::rerank::SortOptions {
                model: model.clone(),
                comparison_budget: budget,
                top_k,
                counterbalance: !no_counterbalance,
                two_sided,
                also_by,
                prune_p_topk_below: prune_below,
                prompt_template_slug: template,
                comparison_concurrency: concurrency,
                ..Default::default()
            };
            let criterion = if elaborate {
                let rubric = llmsort::rerank::elaborate_criterion(
                    gateway.as_ref(),
                    model.as_deref(),
                    &by,
                    Attribution::new("cardinal::sort::elaborate"),
                )
                .await?;
                if !quiet {
                    eprintln!(
                        "elaborated criterion ({}, ${:.4}):
{}
",
                        rubric.model_used,
                        rubric.provider_cost_nanodollars as f64 / 1e9,
                        rubric.elaborated
                    );
                }
                rubric.elaborated
            } else {
                by.clone()
            };
            let mut sorted =
                llmsort::rerank::sort_documents(documents, &criterion, execution, opts).await?;

            drop(trace_sink);
            if let Some(worker) = trace_worker {
                worker.join()?;
            }

            // A sort where every comparison failed or was refused is not a
            // sort; refuse to emit uninformative output on stdout.
            if sorted.meta.comparisons_attempted > 0 && sorted.meta.comparisons_used == 0 {
                return Err(format!(
                    "all {} comparison attempts failed ({} refused); output would be \
                     uninformative. Re-run with --trace <path> to see per-comparison \
                     errors (bad model slug and invalid API key are the usual causes).",
                    sorted.meta.comparisons_attempted, sorted.meta.comparisons_refused,
                )
                .into());
            }

            if reverse {
                sorted.items.reverse();
            }
            let stdout = io::stdout();
            let mut out = stdout.lock();
            render_sorted(&mut out, &sorted, format, scores)?;

            if !quiet {
                let meta = &sorted.meta;
                let cost_usd = meta.provider_cost_nanodollars as f64 / 1e9;
                let estimate = if meta.provider_cost_is_estimate {
                    "~"
                } else {
                    ""
                };
                let evidence = if meta.evidence_judgements > 0 {
                    let residual = meta
                        .evidence_order_residual_mean_abs
                        .map(|r| format!(", order-residual {r:.3} nats"))
                        .unwrap_or_default();
                    format!(
                        " · evidence: {}/{} logprob-mode, visible {:.2}{residual}",
                        meta.logprob_mode_judgements,
                        meta.evidence_judgements,
                        meta.evidence_visible_mass_mean.unwrap_or(0.0)
                    )
                } else {
                    String::new()
                };
                let frustration = meta
                    .judgement_frustration_mean
                    .map(|f| format!(" · frustration {f:.3}"))
                    .unwrap_or_default();
                let flips = if meta.pairs_counterbalanced > 0 {
                    format!(
                        " · order flips: {}/{}",
                        meta.position_flips, meta.pairs_counterbalanced
                    )
                } else {
                    String::new()
                };
                eprintln!(
                    "sorted {} items by \"{by}\" · {} comparisons ({} cached, {} refused) · {estimate}${cost_usd:.4}{flips}{evidence}{frustration} · stop: {}",
                    sorted.items.len(),
                    meta.comparisons_used,
                    meta.comparisons_cached,
                    meta.comparisons_refused,
                    serde_json::to_value(meta.stop_reason)?.as_str().unwrap_or("unknown"),
                );
                // Error budget, experimentalist-style: statistical and
                // systematic components side by side, each in its native
                // unit — never silently pooled.
                {
                    let stat = if sorted.items.is_empty() {
                        None
                    } else {
                        Some(
                            sorted.items.iter().map(|i| i.latent_std).sum::<f64>()
                                / sorted.items.len() as f64,
                        )
                    };
                    let mut parts: Vec<String> = Vec::new();
                    if let Some(stat) = stat {
                        parts.push(format!("stat ±{stat:.3} (posterior, mean)"));
                    }
                    if let Some(residual) = meta.evidence_order_residual_mean_abs {
                        parts.push(format!("syst order {residual:.3} nats/pair"));
                    }
                    if let Some(hcr) = meta.judgement_frustration_mean {
                        parts.push(format!("syst cyclic {:.1}% of energy", hcr * 100.0));
                    }
                    if meta.topk_error > 0.0 {
                        parts.push(format!(
                            "rank risk {:.3} (top-k flip probability)",
                            meta.topk_error
                        ));
                    }
                    if parts.len() > 1 {
                        eprintln!("error budget: {}", parts.join(" · "));
                    }
                }
                for probe in &sorted.probes {
                    let kind = match probe.kind {
                        llmsort::rerank::SortProbeKind::Opposite => "opposite",
                        llmsort::rerank::SortProbeKind::Paraphrase => "paraphrase",
                    };
                    match probe.consistency {
                        Some(c) => {
                            let verdict = if c >= 0.7 {
                                "consistent"
                            } else if c >= 0.3 {
                                "shaky"
                            } else {
                                "INCOHERENT for this judge"
                            };
                            eprintln!(
                                "probe [{kind}] \"{}\": consistency {c:+.2} — {verdict}",
                                probe.prompt
                            );
                        }
                        None => eprintln!(
                            "probe [{kind}] \"{}\": not enough shared scores to assess",
                            probe.prompt
                        ),
                    }
                }
            }
        }
        Commands::Weigh {
            goal,
            attributes,
            propose,
            model,
            template,
            budget,
            seed,
            cache,
            json,
        } => {
            if attributes.len() + propose.unwrap_or(0) < 2 {
                return Err("need at least 2 attributes: --attribute name=description \
                     entries and/or --propose <n>"
                    .into());
            }
            require_openrouter_key()?;
            let gateway = Arc::new(ProviderGateway::from_env(Arc::new(NoopUsageSink))?);

            let mut parsed: Vec<(String, String)> = attributes
                .iter()
                .map(|raw| match raw.split_once('=') {
                    Some((name, text)) => (name.trim().to_string(), text.trim().to_string()),
                    None => (raw.trim().to_string(), raw.trim().to_string()),
                })
                .collect();
            if let Some(count) = propose {
                let (proposed, usage) = llmsort::rerank::propose_for_goal(
                    gateway.as_ref(),
                    model.as_deref().unwrap_or("openai/gpt-5.4-mini"),
                    &goal,
                    count,
                    Attribution::new("cardinal::weigh::propose"),
                )
                .await?;
                eprintln!(
                    "proposed {} considerations for the goal (${:.4}):",
                    proposed.len(),
                    usage.cost_nanodollars as f64 / 1e9
                );
                for c in &proposed {
                    eprintln!("  - {c}");
                }
                for c in proposed {
                    let dup = parsed
                        .iter()
                        .any(|(name, text)| name.eq_ignore_ascii_case(&c) || text == &c);
                    if !dup {
                        parsed.push((c.clone(), c));
                    }
                }
            }
            if parsed.len() < 2 {
                return Err("fewer than 2 distinct attributes after proposal".into());
            }
            let documents: Vec<llmsort::rerank::RerankDocument> = parsed
                .iter()
                .map(|(name, text)| llmsort::rerank::RerankDocument {
                    id: name.clone(),
                    text: if name == text {
                        name.clone()
                    } else {
                        format!("{name}: {text}")
                    },
                })
                .collect();
            let criterion = format!(
                "importance for achieving this goal: {goal}. Judge how much more one \
                 consideration matters than the other for that goal specifically — not in \
                 general, not for other goals."
            );
            let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
            let cache_store = SqlitePairwiseCache::new(cache_path)?;
            let execution =
                llmsort::rerank::RerankExecution::new(gateway, Attribution::new("cardinal::weigh"))
                    .cache(&cache_store)
                    .run_options(RerankRunOptions {
                        rng_seed: Some(seed),
                        cache_only: false,
                    });
            let opts = llmsort::rerank::SortOptions {
                model,
                comparison_budget: budget,
                prompt_template_slug: template,
                ..Default::default()
            };
            let sorted =
                llmsort::rerank::sort_documents(documents, &criterion, execution, opts).await?;

            // Softmax of log-latents: normalized ratio-scale weights.
            let max_latent = sorted
                .items
                .iter()
                .map(|item| item.latent_mean)
                .fold(f64::NEG_INFINITY, f64::max);
            let unnormalized: Vec<f64> = sorted
                .items
                .iter()
                .map(|item| (item.latent_mean - max_latent).exp())
                .collect();
            let z: f64 = unnormalized.iter().sum();

            if json {
                let weights: Vec<serde_json::Value> = sorted
                    .items
                    .iter()
                    .zip(unnormalized.iter())
                    .map(|(item, u)| {
                        serde_json::json!({
                            "attribute": item.id,
                            "weight": u / z,
                            "latent_mean": item.latent_mean,
                            "latent_std": item.latent_std,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "goal": goal,
                        "weights": weights,
                    }))?
                );
            } else {
                for (item, u) in sorted.items.iter().zip(unnormalized.iter()) {
                    println!(
                        "{:>7.4}  {}  ({:+.3} ± {:.3})",
                        u / z,
                        item.id,
                        item.latent_mean,
                        item.latent_std
                    );
                }
            }
            let meta = &sorted.meta;
            eprintln!(
                "weighed {} attributes for goal \"{goal}\" · {} comparisons ({} cached) · ${:.4}",
                sorted.items.len(),
                meta.comparisons_used,
                meta.comparisons_cached,
                meta.provider_cost_nanodollars as f64 / 1e9,
            );
        }
        Commands::Distinguish {
            input,
            focus,
            by,
            propose,
            model,
            budget,
            seed,
            cache,
            json,
        } => {
            let raw = read_sort_input(input.as_deref())?;
            let documents = parse_sort_items(&raw)?;
            if documents.len() < 3 {
                return Err("distinguish requires at least 3 items (the focal item \
                     and at least 2 peers)"
                    .into());
            }
            require_openrouter_key()?;

            // Resolve the focal item: 1-based line number, id, or exact text.
            let focal_id = if let Ok(index) = focus.parse::<usize>() {
                if index == 0 || index > documents.len() {
                    return Err(
                        format!("--focus {index} out of range (1..={})", documents.len()).into(),
                    );
                }
                documents[index - 1].id.clone()
            } else if let Some(doc) = documents.iter().find(|d| d.id == focus) {
                doc.id.clone()
            } else if let Some(doc) = documents.iter().find(|d| d.text == focus) {
                doc.id.clone()
            } else {
                return Err(format!(
                    "--focus \"{focus}\" matched no item (use a 1-based line \
                     number, an item id, or the exact item text)"
                )
                .into());
            };

            let gateway = Arc::new(ProviderGateway::from_env(Arc::new(NoopUsageSink))?);

            let mut candidates = by;
            let propose_count = propose.unwrap_or(if candidates.is_empty() { 5 } else { 0 });
            if propose_count > 0 {
                let (proposed, usage) = llmsort::rerank::propose_distinguishing(
                    gateway.as_ref(),
                    model.as_deref().unwrap_or("openai/gpt-5.4-mini"),
                    &documents,
                    &focal_id,
                    propose_count,
                    Attribution::new("cardinal::distinguish::propose"),
                )
                .await?;
                eprintln!(
                    "proposed {} distinguishing attributes (${:.4}):",
                    proposed.len(),
                    usage.cost_nanodollars as f64 / 1e9
                );
                for c in &proposed {
                    eprintln!("  - {c}");
                }
                for c in proposed {
                    if !candidates.iter().any(|have| have.eq_ignore_ascii_case(&c)) {
                        candidates.push(c);
                    }
                }
            }
            if candidates.is_empty() {
                return Err(
                    "no candidate attributes: pass --by \"<attribute>\" (repeatable) \
                     and/or --propose <n>"
                        .into(),
                );
            }

            let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
            let cache_store = SqlitePairwiseCache::new(cache_path)?;
            let execution = llmsort::rerank::RerankExecution::new(
                gateway,
                Attribution::new("cardinal::distinguish"),
            )
            .cache(&cache_store)
            .run_options(RerankRunOptions {
                rng_seed: Some(seed),
                cache_only: false,
            });
            let opts = llmsort::rerank::ExplainOptions {
                model,
                comparison_budget: budget,
                ..Default::default()
            };
            let focal_text = documents
                .iter()
                .find(|d| d.id == focal_id)
                .map(|d| d.text.clone())
                .unwrap_or_default();
            let profile = llmsort::rerank::differentiation_profile(
                documents, &focal_id, candidates, execution, opts,
            )
            .await?;

            if json {
                println!("{}", serde_json::to_string_pretty(&profile)?);
            } else {
                let shown: String = focal_text.chars().take(72).collect();
                println!("focal: {focal_id}  {shown}");
                for attr in &profile.attributes {
                    println!(
                        "p{:<4.0} z{:+.2}  {}  ({:+.3} ± {:.3})",
                        attr.percentile * 100.0,
                        attr.z_score,
                        attr.prompt,
                        attr.latent_mean,
                        attr.latent_std
                    );
                }
                if let Some(best) = profile.attributes.first() {
                    if best.percentile >= 0.7 {
                        println!(
                            "propagate under: \"{}\" — focal at p{:.0}, z {:+.2}",
                            best.prompt,
                            best.percentile * 100.0,
                            best.z_score
                        );
                    } else {
                        println!(
                            "no measured standout: best attribute \"{}\" only places \
                             the focal item at p{:.0}",
                            best.prompt,
                            best.percentile * 100.0
                        );
                    }
                }
            }
            let meta = &profile.meta;
            eprint!(
                "profiled {} attributes · {} comparisons ({} cached) · ${:.4}",
                profile.attributes.len(),
                meta.comparisons_used,
                meta.comparisons_cached,
                meta.provider_cost_nanodollars as f64 / 1e9,
            );
            if let Some(f) = meta.judgement_frustration_mean {
                eprint!(" · frustration {f:.3}");
            }
            eprintln!();
        }
        Commands::Slate {
            item,
            note,
            stakeholders,
            stakeholder_count,
            per,
            model,
            json,
        } => {
            require_openrouter_key()?;
            let item_text = read_item_arg(&item)?;
            let note_text = note.as_deref().map(read_item_arg).transpose()?;
            let stakeholder_list: Vec<String> = stakeholders
                .as_deref()
                .map(|s| s.split(',').map(|x| x.trim().to_string()).collect())
                .unwrap_or_default();
            let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
            let report = llmsort_experiments::propose_slate(
                &gateway,
                model.as_deref().unwrap_or("openai/gpt-5.4-mini"),
                &item_text,
                note_text.as_deref(),
                stakeholder_list,
                stakeholder_count,
                per,
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("stakeholders: {}", report.stakeholders.join(" · "));
                for entry in &report.slate {
                    println!(
                        "  [{}] {}  ({})",
                        entry.backers.len(),
                        entry.attribute,
                        entry.backers.join(", ")
                    );
                }
            }
            eprintln!(
                "slate: {} attributes from {} stakeholders · {} calls · ${:.4}",
                report.slate.len(),
                report.stakeholders.len(),
                report.calls,
                report.cost_nanodollars as f64 / 1e9,
            );
        }
        Commands::Canonize {
            input,
            by,
            propose,
            judges,
            accepted,
            budget,
            seed,
            cache,
            json,
        } => {
            let raw = read_sort_input(input.as_deref())?;
            let entities = parse_sort_items(&raw)?;
            let judge_list: Vec<String> = judges
                .split(',')
                .map(|m| m.trim().to_string())
                .filter(|m| !m.is_empty())
                .collect();
            require_openrouter_key()?;
            let gateway = Arc::new(ProviderGateway::from_env(Arc::new(NoopUsageSink))?);

            let mut candidates = vec![by.clone()];
            if propose > 0 {
                let (proposed, usage) = llmsort::rerank::propose_rewordings(
                    gateway.as_ref(),
                    judge_list
                        .first()
                        .map(String::as_str)
                        .unwrap_or("openai/gpt-5.4-mini"),
                    &by,
                    propose,
                    Attribution::new("cardinal::canonize::propose"),
                )
                .await?;
                eprintln!(
                    "proposed {} rewordings (${:.4}):",
                    proposed.len(),
                    usage.cost_nanodollars as f64 / 1e9
                );
                for c in &proposed {
                    eprintln!("  - {c}");
                }
                for c in proposed {
                    if !candidates.iter().any(|have| have.eq_ignore_ascii_case(&c)) {
                        candidates.push(c);
                    }
                }
            }

            let sorts = llmsort_experiments::planned_sorts(
                accepted.len(),
                candidates.len(),
                judge_list.len(),
            );
            match budget {
                Some(total) => eprintln!(
                    "canonize: {sorts} sorts ({} accepted + {} candidates × {} judges) · \
                     budget {total} total → {} comparisons per sort",
                    accepted.len(),
                    candidates.len(),
                    judge_list.len(),
                    total / sorts.max(1),
                ),
                None => eprintln!(
                    "canonize: {sorts} sorts ({} accepted + {} candidates × {} judges) · \
                     per-sort default budget (4·n each)",
                    accepted.len(),
                    candidates.len(),
                    judge_list.len(),
                ),
            }
            let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
            let cache_store = SqlitePairwiseCache::new(cache_path)?;
            let report = llmsort_experiments::canonize(
                gateway,
                Some(&cache_store as &dyn llmsort::cache::PairwiseCache),
                entities,
                candidates,
                accepted,
                llmsort_experiments::CanonizeOptions {
                    judges: judge_list,
                    comparison_budget: budget,
                    seed,
                },
            )
            .await?;

            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{:<7} {:>7} {:>7}  wording", "trans", "signal", "redund");
                for c in &report.candidates {
                    let t = c
                        .transmissibility
                        .map(|t| format!("{t:+.3}"))
                        .unwrap_or_else(|| "-".into());
                    let r = c
                        .redundancy
                        .map(|r| format!("{r:.3}"))
                        .unwrap_or_else(|| "-".into());
                    println!("{t:<7} {:>7.3} {r:>7}  {}", c.signal_nats, c.prompt);
                }
                if let Some(best) = report.candidates.first() {
                    if let Some(t) = best.transmissibility {
                        println!(
                            "canonical candidate: \"{}\" — transmissibility {t:+.3} across {} judges",
                            best.prompt,
                            report.judges.len()
                        );
                    }
                }
            }
            eprintln!(
                "canonize: {} candidates x {} judges · {} comparisons · ${:.4}",
                report.candidates.len(),
                report.judges.len(),
                report.comparisons_used,
                report.cost_nanodollars as f64 / 1e9,
            );
        }
        Commands::Anp {
            input,
            goal,
            attributes,
            propose,
            alpha,
            model,
            budget,
            seed,
            cache,
            json,
        } => {
            let raw = read_sort_input(input.as_deref())?;
            let alternatives = parse_sort_items(&raw)?;
            require_openrouter_key()?;
            let gateway = Arc::new(ProviderGateway::from_env(Arc::new(NoopUsageSink))?);

            let mut criteria: Vec<(String, String)> = attributes
                .iter()
                .map(|raw| match raw.split_once('=') {
                    Some((name, text)) => (name.trim().to_string(), text.trim().to_string()),
                    None => (raw.trim().to_string(), raw.trim().to_string()),
                })
                .collect();
            if let Some(count) = propose {
                let (proposed, usage) = llmsort::rerank::propose_for_goal(
                    gateway.as_ref(),
                    model.as_deref().unwrap_or("openai/gpt-5.4-mini"),
                    &goal,
                    count,
                    Attribution::new("cardinal::anp::propose"),
                )
                .await?;
                eprintln!(
                    "proposed {} criteria (${:.4}):",
                    proposed.len(),
                    usage.cost_nanodollars as f64 / 1e9
                );
                for cand in &proposed {
                    eprintln!("  - {cand}");
                }
                for cand in proposed {
                    if !criteria
                        .iter()
                        .any(|(name, _)| name.eq_ignore_ascii_case(&cand))
                    {
                        criteria.push((cand.clone(), cand));
                    }
                }
            }
            let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
            let cache_store = SqlitePairwiseCache::new(cache_path)?;
            let report = llmsort_experiments::anp(
                gateway,
                Some(&cache_store as &dyn llmsort::cache::PairwiseCache),
                &goal,
                &criteria,
                alternatives,
                llmsort_experiments::AnpOptions {
                    model,
                    alpha,
                    comparison_budget: budget,
                    seed,
                },
            )
            .await?;

            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("criteria (direct → limiting, Δ = network correction):");
                for criterion in &report.criteria {
                    println!(
                        "  {:>6.3} → {:>6.3}  ({:+.3})  {}",
                        criterion.direct_weight,
                        criterion.limiting_weight,
                        criterion.network_delta,
                        criterion.name
                    );
                }
                println!("alternatives by limiting priority:");
                for alternative in &report.alternatives {
                    println!(
                        "  {:>6.3}  {}",
                        alternative.limiting_priority, alternative.id
                    );
                }
            }
            eprintln!(
                "anp: {} criteria x {} alternatives · alpha {alpha} · {} comparisons · ${:.4} · limit {} iterations ({})",
                report.criteria.len(),
                report.alternatives.len(),
                report.comparisons_used,
                report.cost_nanodollars as f64 / 1e9,
                report.iterations,
                if report.converged { "converged" } else { "NOT CONVERGED" },
            );
        }
        Commands::Probe {
            file,
            by,
            model,
            probes,
            template,
            concurrency,
            pairs,
            out,
            no_cache,
            cache,
        } => {
            require_openrouter_key()?;
            let raw = std::fs::read_to_string(&file)?;
            let entities: Vec<String> = raw
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect();
            if entities.len() < 2 {
                return Err("need at least 2 entities to probe pairs".into());
            }
            let n = entities.len();
            let mut ring: Vec<(usize, usize)> = (0..n).map(|i| (i, (i + 1) % n)).collect();
            if n == 2 {
                ring.truncate(1);
            }
            if let Some(cap) = pairs {
                ring.truncate(cap.max(1));
            }
            let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
            let cache_store = if no_cache {
                None
            } else {
                let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
                Some(SqlitePairwiseCache::new(cache_path)?)
            };
            let cache_ref = cache_store
                .as_ref()
                .map(|c| c as &dyn llmsort::cache::PairwiseCache);
            let report = llmsort_experiments::run_probe_battery(
                &gateway,
                cache_ref,
                &entities,
                &ring,
                &by,
                llmsort_experiments::ProbeBatteryOptions {
                    model,
                    template,
                    probes,
                    concurrency,
                },
            )
            .await?;
            print!("{}", llmsort_experiments::render_probe_report(&report));
            if let Some(path) = out {
                std::fs::write(&path, serde_json::to_string_pretty(&report)?)?;
                eprintln!("report written to {}", path.display());
            }
        }
        Commands::Bench {
            models,
            template,
            concurrency,
            pool,
            scale_n,
            battery_seed,
            battery,
            battery_out,
            out,
            json,
            no_cache,
            cache,
        } => {
            require_openrouter_key()?;
            let model_list: Vec<String> = models
                .split(',')
                .map(|m| m.trim().to_string())
                .filter(|m| !m.is_empty())
                .collect();
            if model_list.is_empty() {
                return Err("no models given: --models a/b,c/d".into());
            }
            let battery_spec = if let Some(path) = battery {
                let spec: llmsort_experiments::BatterySpec =
                    serde_json::from_str(&std::fs::read_to_string(&path)?)?;
                spec.validate()?;
                spec
            } else if let Some(path) = pool {
                let pool: llmsort_experiments::EntityPool =
                    serde_json::from_str(&std::fs::read_to_string(&path)?)?;
                let scale =
                    llmsort_experiments::BatteryScale::for_n(scale_n.unwrap_or(pool.items.len()));
                llmsort_experiments::BatterySpec::generate(&pool, &scale, battery_seed)?
            } else {
                llmsort_experiments::BatterySpec::v1()
            };
            if let Some(path) = battery_out {
                std::fs::write(&path, serde_json::to_string_pretty(&battery_spec)?)?;
                eprintln!("battery spec written to {}", path.display());
            }
            eprintln!(
                "battery {} · {} entities · {} core pairs · {} calls/model",
                battery_spec.slug,
                battery_spec.corpus.len(),
                battery_spec.core_pairs.len(),
                battery_spec.calls_per_run(),
            );
            let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
            let cache_store = if no_cache {
                None
            } else {
                let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
                Some(SqlitePairwiseCache::new(cache_path)?)
            };
            let cache_ref = cache_store
                .as_ref()
                .map(|c| c as &dyn llmsort::cache::PairwiseCache);

            let mut out_file = match out.as_ref() {
                Some(path) => Some(std::io::BufWriter::new(std::fs::File::create(path)?)),
                None => None,
            };
            let mut reports = Vec::new();
            for model in &model_list {
                eprintln!("benchmarking {model} ...");
                let report = llmsort_experiments::run_judge_bench(
                    &gateway,
                    cache_ref,
                    llmsort_experiments::JudgeBenchOptions {
                        model: model.clone(),
                        template: template.clone(),
                        concurrency,
                        battery: battery_spec.clone(),
                    },
                )
                .await?;
                eprint!("{}", llmsort_experiments::render_bench_report(&report));
                if let Some(file) = out_file.as_mut() {
                    use std::io::Write as _;
                    writeln!(file, "{}", serde_json::to_string(&report)?)?;
                }
                reports.push(report);
            }
            if let Some(mut file) = out_file {
                use std::io::Write as _;
                file.flush()?;
            }

            reports.sort_by(|a, b| {
                b.judge_score
                    .unwrap_or(f64::NEG_INFINITY)
                    .partial_cmp(&a.judge_score.unwrap_or(f64::NEG_INFINITY))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            if json {
                let board: Vec<serde_json::Value> = reports
                    .iter()
                    .map(|r| {
                        serde_json::json!({
                            "model": r.model,
                            "judge_score": r.judge_score,
                            "signal_nats": r.signal.value,
                            "coherence": r.coherence,
                            "coherence_harmonic": r.coherence_harmonic,
                            "order_flip_rate": r.order_flip.value,
                            "order_residual_nats": r.order_residual.value,
                            "frustration": r.frustration.value,
                            "spin_survival": r.spin_survival.value,
                            "susceptibility_nats": r.susceptibility.value,
                            "polarity_spearman": r.polarity.value,
                            "paraphrase_spearman": r.paraphrase.value,
                            "null_bias_nats": r.null_bias.value,
                            "nuisance_drift_nats": r.nuisance.value,
                            "refusals": r.refusals,
                            "cost_nanodollars": r.cost_nanodollars,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&board)?);
            } else {
                println!(
                    "{:<4} {:<34} {:>7} {:>7} {:>9} {:>6} {:>6} {:>7} {:>7} {:>8}",
                    "rank",
                    "model",
                    "JUDGE",
                    "signal",
                    "coherence",
                    "flip",
                    "curl",
                    "spin",
                    "pol ρ",
                    "cost $"
                );
                for (idx, r) in reports.iter().enumerate() {
                    let f =
                        |v: Option<f64>| v.map(|x| format!("{x:.3}")).unwrap_or_else(|| "-".into());
                    println!(
                        "{:<4} {:<34} {:>7} {:>7} {:>9} {:>6} {:>6} {:>7} {:>7} {:>8.4}",
                        idx + 1,
                        r.model,
                        f(r.judge_score),
                        f(r.signal.value),
                        f(r.coherence),
                        f(r.order_flip.value),
                        f(r.frustration.value),
                        f(r.spin_survival.value),
                        f(r.polarity.value),
                        r.cost_nanodollars as f64 / 1e9,
                    );
                }
            }
        }
        Commands::Calibrate { models, nulls, out } => {
            require_openrouter_key()?;
            let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
            let null_texts = [
                "The quick brown fox jumps over the lazy dog.",
                "In the beginning there was only the sea and the sky.",
                "fn main() { println!(\"hello\"); }",
                "Buy milk, eggs, and two lemons on the way home.",
                "We hold these truths to be self-evident.",
                "The mitochondria is the powerhouse of the cell.",
                "A minor seventh chord contains four distinct pitches.",
                "Snow fell quietly on the empty parking lot.",
            ];
            let attribute =
                llmsort::seriate::Attribute::new("quality", "overall quality of the writing");
            let instrument = llmsort::seriate::instrument::ratio_letter::RatioLetterInstrument;
            use llmsort::seriate::instrument::Instrument as _;

            println!(
                "{:<40} {:>7} {:>7} {:>7} {:>10} {:>8}",
                "model", "P(A)", "P(par)", "P(B)", "bias-nats", "cost$"
            );
            let mut reports: Vec<serde_json::Value> = Vec::new();
            for model in models.split(',').map(str::trim).filter(|m| !m.is_empty()) {
                let mut p_a_sum = 0.0f64;
                let mut p_par_sum = 0.0f64;
                let mut p_b_sum = 0.0f64;
                let mut bias_abs_sum = 0.0f64;
                let mut count = 0usize;
                let mut cost: i64 = 0;
                let mut logprob_mode = 0usize;
                for text in null_texts.iter().take(nulls as usize) {
                    let entity = llmsort::seriate::Entity::new(*text);
                    let rendered = instrument.render(&attribute, &entity, &entity);
                    let mut chat = llmsort::gateway::ChatRequest::new(
                        llmsort::gateway::ChatModel::parse(model),
                        vec![
                            llmsort::gateway::Message::system(rendered.system.clone()),
                            llmsort::gateway::Message::user(rendered.user.clone()),
                        ],
                        Attribution::new("cardinal::calibrate"),
                    )
                    .max_tokens(16);
                    chat = chat.with_logprobs(20);
                    let response = match gateway.chat(chat.clone()).await {
                        Ok(response) => response,
                        Err(err) if format!("{err}").to_ascii_lowercase().contains("logprob") => {
                            let mut plain = chat.clone();
                            plain.logprobs = false;
                            plain.top_logprobs = None;
                            gateway.chat(plain).await?
                        }
                        Err(err) => return Err(err.into()),
                    };
                    cost += response.cost_nanodollars;
                    let Ok(parsed) =
                        instrument.parse(&response.content, response.output_logprobs.as_deref())
                    else {
                        continue;
                    };
                    if parsed.mode == llmsort::seriate::AcquisitionMode::Logprob {
                        logprob_mode += 1;
                    }
                    if let Some((p_a, p_par, p_b)) = parsed.evidence.directional_summary() {
                        p_a_sum += p_a;
                        p_par_sum += p_par;
                        p_b_sum += p_b;
                        if let Some((mean, _)) = parsed.evidence.log_ratio_moments() {
                            bias_abs_sum += mean.abs();
                        }
                        count += 1;
                    }
                }
                if count == 0 {
                    println!("{model:<40} no parseable null judgements");
                    continue;
                }
                let n = count as f64;
                println!(
                    "{:<40} {:>7.3} {:>7.3} {:>7.3} {:>10.4} {:>8.4}",
                    model,
                    p_a_sum / n,
                    p_par_sum / n,
                    p_b_sum / n,
                    bias_abs_sum / n,
                    cost as f64 / 1e9,
                );
                reports.push(serde_json::json!({
                    "model": model,
                    "nulls": count,
                    "logprob_mode": logprob_mode,
                    "p_slot_a": p_a_sum / n,
                    "p_parity": p_par_sum / n,
                    "p_slot_b": p_b_sum / n,
                    "mean_abs_bias_nats": bias_abs_sum / n,
                    "cost_nanodollars": cost,
                }));
            }
            eprintln!(
                "null-pair calibration: identical text in both slots — a perfect judge \
                 answers parity; directional mass is pure position+letter artifact"
            );
            if let Some(path) = out {
                use std::io::Write as _;
                let mut file = File::create(&path)?;
                for report in &reports {
                    writeln!(file, "{report}")?;
                }
                eprintln!("wrote {} reports to {}", reports.len(), path.display());
            }
        }
        Commands::Judge {
            item_a,
            item_b,
            by,
            model,
            template,
            show_prompt,
            json,
            no_cache,
            cache,
            spin,
            sweep,
            orbit,
            wordings,
            draws,
            temperature,
            consortium,
            packets_out,
        } => {
            let text_a = read_item_arg(&item_a)?;
            let text_b = read_item_arg(&item_b)?;
            let model = model
                .as_deref()
                .unwrap_or("openai/gpt-5.4-mini")
                .to_string();

            if let Some(consortium) = consortium {
                let models: Vec<String> = consortium
                    .split(',')
                    .map(|m| m.trim().to_string())
                    .filter(|m| !m.is_empty())
                    .collect();
                require_openrouter_key()?;
                let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
                let cache_store = if no_cache {
                    None
                } else {
                    let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
                    Some(SqlitePairwiseCache::new(cache_path)?)
                };
                let cache_ref = cache_store
                    .as_ref()
                    .map(|c| c as &dyn llmsort::cache::PairwiseCache);
                // Stable entity labels: packets accrete across runs by
                // id + content hash, so @path items keep their file stem
                // and literals get a content-derived label.
                let label = |arg: &str, text: &str| -> String {
                    arg.strip_prefix('@')
                        .and_then(|p| {
                            std::path::Path::new(p)
                                .file_stem()
                                .map(|s| s.to_string_lossy().into_owned())
                        })
                        .unwrap_or_else(|| {
                            llmsort::packet::entity_text_hash(text)[..12].to_string()
                        })
                };
                let id_a = label(&item_a, &text_a);
                let id_b = label(&item_b, &text_b);
                let created = chrono::Utc::now().to_rfc3339();
                let report = llmsort::rerank::consortium_verdict(
                    &gateway,
                    cache_ref,
                    &models,
                    &by,
                    (&id_a, &text_a),
                    (&id_b, &text_b),
                    &template,
                    &created,
                    Attribution::new("cardinal::judge::consortium"),
                )
                .await?;
                let written = if let Some(dir) = packets_out.as_ref() {
                    std::fs::create_dir_all(dir)?;
                    let mut paths = Vec::new();
                    for packet in &report.packets {
                        let path = dir.join(format!(
                            "packet-{}-{}.json",
                            packet.judge.replace('/', "-"),
                            &packet.id().0[..12],
                        ));
                        std::fs::write(&path, serde_json::to_string_pretty(packet)?)?;
                        paths.push(path);
                    }
                    paths
                } else {
                    Vec::new()
                };
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    println!(
                        "{:<34} {:>8} {:>9}  top bias",
                        "judge", "belief", "coherence"
                    );
                    for j in &report.judges {
                        match (j.belief, j.coherence, &j.top_bias) {
                            (Some(b), Some(c), Some((name, coef))) => {
                                println!("{:<34} {b:+8.3} {c:9.3}  {name} {coef:+.3}", j.model)
                            }
                            _ => println!(
                                "{:<34} orbit incomplete ({} refusals) — excluded",
                                j.model, j.refusals
                            ),
                        }
                    }
                    match (report.belief, report.ratio) {
                        (Some(b), Some(r)) => {
                            let toward = if b >= 0.0 { &id_a } else { &id_b };
                            println!(
                                "belief (fused, toward {toward}): {:+.3} nats · ratio {r:.2}×",
                                b
                            );
                            let spread = report
                                .judge_spread_nats
                                .map(|s| format!("{s:.3}"))
                                .unwrap_or_else(|| "n/a (1 judge)".into());
                            let bias = report
                                .orbit_bias_rms
                                .map(|s| format!("{s:.3}"))
                                .unwrap_or_else(|| "n/a".into());
                            let unanimity = match report.direction_unanimous {
                                Some(true) => format!(
                                    "unanimous ({}/{})",
                                    report.usable_judges, report.usable_judges
                                ),
                                Some(false) => "SPLIT".into(),
                                None => "n/a".into(),
                            };
                            println!(
                                "error budget: syst orbit-bias rms {bias} · syst judge spread \
                                 {spread} (nats) · direction {unanimity}"
                            );
                        }
                        _ => println!("no usable judge completed its orbit — no verdict"),
                    }
                    if let Some(matrix) = &report.residual_correlation {
                        let rows: Vec<String> = matrix
                            .iter()
                            .map(|row| {
                                row.iter()
                                    .map(|v| format!("{v:+.2}"))
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            })
                            .collect();
                        println!(
                            "shared-bias correlation (orbit residuals, n = 8 cells): [{}]",
                            rows.join(" | ")
                        );
                    }
                    for path in &written {
                        println!("packet: {}", path.display());
                    }
                }
                eprintln!(
                    "{} judges ({} usable) · {} comparisons ({} cached) · ${:.4}",
                    report.judges.len(),
                    report.usable_judges,
                    report.comparisons,
                    report.comparisons_cached,
                    report.cost_nanodollars as f64 / 1e9,
                );
                return Ok(());
            }

            if let Some(k) = draws {
                require_openrouter_key()?;
                let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
                let report = llmsort::rerank::nonce_draws(
                    &gateway,
                    &model,
                    &template,
                    &by,
                    ("A", &text_a),
                    ("B", &text_b),
                    k,
                    temperature,
                    7,
                    Attribution::new("cardinal::judge::draws"),
                )
                .await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    for (i, d) in report.draws.iter().enumerate() {
                        match d {
                            Some(m) => println!("draw {i}: {m:+.3} nats"),
                            None => println!("draw {i}: refused"),
                        }
                    }
                    match (report.mean, report.sigma_w) {
                        (Some(m), Some(s)) => println!(
                            "mean {m:+.3} nats · sigma_w {s:.3} (n = {})",
                            report.comparisons - report.refusals
                        ),
                        (Some(m), None) => println!("mean {m:+.3} nats (single usable draw)"),
                        _ => println!("no usable draws"),
                    }
                    println!(
                        "cache: {} of {} input tokens billed as cached",
                        report.cache_read_tokens_total, report.input_tokens_total
                    );
                }
                eprintln!(
                    "{} draws ({} refused) · ${:.4}",
                    report.comparisons,
                    report.refusals,
                    report.cost_nanodollars as f64 / 1e9,
                );
                return Ok(());
            }

            if orbit {
                require_openrouter_key()?;
                let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
                let cache_store = if no_cache {
                    None
                } else {
                    let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
                    Some(SqlitePairwiseCache::new(cache_path)?)
                };
                let cache_ref = cache_store
                    .as_ref()
                    .map(|c| c as &dyn llmsort::cache::PairwiseCache);
                let report = llmsort::rerank::orbit_transform(
                    &gateway,
                    cache_ref,
                    &model,
                    &by,
                    ("A", &text_a),
                    ("B", &text_b),
                    &template,
                    Attribution::new("cardinal::judge::orbit"),
                )
                .await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else if report.refusals > 0 {
                    println!(
                        "orbit incomplete: {} refusals in 8 variants — no transform",
                        report.refusals
                    );
                } else {
                    let total: f64 = report.energies.iter().sum();
                    for (idx, name) in llmsort::rerank::CHARACTERS.iter().enumerate() {
                        println!(
                            "{name:<26} {:+.3} nats  ({:.1}% of energy)",
                            report.coefficients[idx],
                            100.0 * report.energies[idx] / total.max(1e-12)
                        );
                    }
                    if let Some(c) = report.coherence {
                        println!("coherence (invariant fraction): {c:.3}");
                    }
                    println!("parseval residual: {:.2e}", report.parseval_residual);
                }
                eprintln!(
                    "{} comparisons ({} cached) · ${:.4}",
                    report.comparisons,
                    report.comparisons_cached,
                    report.cost_nanodollars as f64 / 1e9,
                );
                return Ok(());
            }

            if wordings {
                require_openrouter_key()?;
                let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
                let cache_store = if no_cache {
                    None
                } else {
                    let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
                    Some(SqlitePairwiseCache::new(cache_path)?)
                };
                let cache_ref = cache_store
                    .as_ref()
                    .map(|c| c as &dyn llmsort::cache::PairwiseCache);
                let report = llmsort::rerank::wording_invariance(
                    &gateway,
                    cache_ref,
                    &model,
                    &by,
                    ("A", &text_a),
                    ("B", &text_b),
                    Attribution::new("cardinal::judge::wordings"),
                )
                .await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    for r in &report.readings {
                        match r.mean_log_ratio {
                            Some(m) => println!("{:<14} {m:+.3} nats", r.template),
                            None => println!("{:<14} refused", r.template),
                        }
                    }
                    match report.sign_consistent {
                        Some(true) => {
                            println!("inversion: OK — the judge can mirror its own scale")
                        }
                        Some(false) => println!(
                            "inversion: FAILS — asking \"which has less\" flips the belief"
                        ),
                        None => println!("inversion: undetermined"),
                    }
                    if let Some(d) = report.max_disagreement_nats {
                        println!(
                            "max wording disagreement: {d:.3} nats{}",
                            if d > 0.5 {
                                " — numerical framing bias"
                            } else {
                                ""
                            }
                        );
                    }
                }
                eprintln!(
                    "{} comparisons ({} cached) · ${:.4}",
                    report.comparisons,
                    report.comparisons_cached,
                    report.cost_nanodollars as f64 / 1e9,
                );
                return Ok(());
            }

            if sweep {
                require_openrouter_key()?;
                let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
                let cache_store = if no_cache {
                    None
                } else {
                    let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
                    Some(SqlitePairwiseCache::new(cache_path)?)
                };
                let cache_ref = cache_store
                    .as_ref()
                    .map(|c| c as &dyn llmsort::cache::PairwiseCache);
                let report = llmsort::rerank::spin_sweep(
                    &gateway,
                    cache_ref,
                    &model,
                    &template,
                    &by,
                    ("A", &text_a),
                    ("B", &text_b),
                    Attribution::new("cardinal::judge::sweep"),
                )
                .await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    for r in &report.readings {
                        match r.mean_log_ratio {
                            Some(m) => println!("field {:+}: {m:+.3} nats", r.field),
                            None => println!("field {:+}: refused", r.field),
                        }
                    }
                    match (report.chi_slope, report.linearity_r2) {
                        (Some(chi), Some(r2)) => {
                            let even = report
                                .even_response_mean
                                .map(|e| format!(" · even component {e:+.3} nats"))
                                .unwrap_or_default();
                            println!(
                                "response: odd slope {chi:+.3} nats/step · linear R² {r2:.3}{even}"
                            );
                        }
                        _ => println!("response: unmeasurable (refusals)"),
                    }
                    match report.belief_survives_sweep {
                        Some(true) => println!("sign(m) constant over the sweep: yes"),
                        Some(false) => println!("sign(m) constant over the sweep: no"),
                        None => {
                            println!("sign(m) constant over the sweep: undetermined (m(0) = 0)")
                        }
                    }
                }
                eprintln!(
                    "{} comparisons ({} cached) · ${:.4}",
                    report.comparisons,
                    report.comparisons_cached,
                    report.cost_nanodollars as f64 / 1e9,
                );
                return Ok(());
            }

            if spin {
                require_openrouter_key()?;
                let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
                let cache_store = if no_cache {
                    None
                } else {
                    let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
                    Some(SqlitePairwiseCache::new(cache_path)?)
                };
                let cache_ref = cache_store
                    .as_ref()
                    .map(|c| c as &dyn llmsort::cache::PairwiseCache);
                let report = llmsort::rerank::spin_probe(
                    &gateway,
                    cache_ref,
                    &model,
                    &template,
                    &by,
                    ("A", &text_a),
                    ("B", &text_b),
                    Attribution::new("cardinal::judge::spin"),
                )
                .await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    for reading in &report.readings {
                        let label = match reading.framing {
                            llmsort::rerank::SpinFraming::Neutral => "neutral   ",
                            llmsort::rerank::SpinFraming::ProFirst => "pro-A spin",
                            llmsort::rerank::SpinFraming::ProSecond => "pro-B spin",
                        };
                        match reading.mean_log_ratio {
                            Some(m) => {
                                let winner = if m >= 0.0 { "A" } else { "B" };
                                let order = if reading.flipped_by_order {
                                    " · ORDER-FLIPPED"
                                } else {
                                    ""
                                };
                                println!("{label}: {winner} wins · {:+.3} nats{order}", m);
                            }
                            None => println!("{label}: refused"),
                        }
                    }
                    match report.susceptibility_nats {
                        Some(chi) => println!("susceptibility (secant): {chi:+.3} nats/spin"),
                        None => println!("susceptibility: unmeasurable (refusals)"),
                    }
                    match report.belief_survives_spin {
                        Some(true) => println!("sign(m) constant across framings: yes"),
                        Some(false) => println!("sign(m) constant across framings: no"),
                        None => println!("sign(m) constant across framings: undetermined"),
                    }
                }
                eprintln!(
                    "{} comparisons ({} cached) · ${:.4}",
                    report.comparisons,
                    report.comparisons_cached,
                    report.cost_nanodollars as f64 / 1e9,
                );
                return Ok(());
            }

            let spec = llmsort::rerank::PairwiseComparisonSpec {
                model: &model,
                attribute: llmsort::rerank::PairwiseComparisonAttribute {
                    id: "judge",
                    prompt: &by,
                    prompt_template_slug: Some(&template),
                },
                entity_a: llmsort::rerank::PairwiseComparisonEntity {
                    id: "A",
                    text: &text_a,
                },
                entity_b: llmsort::rerank::PairwiseComparisonEntity {
                    id: "B",
                    text: &text_b,
                },
            };

            if show_prompt {
                let rendered = spec.prompt_instance();
                eprintln!(
                    "--- system ---
{}
--- user ---
{}
---",
                    rendered.system, rendered.user
                );
            }

            require_openrouter_key()?;
            let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
            let cache_store = if no_cache {
                None
            } else {
                let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
                Some(SqlitePairwiseCache::new(cache_path)?)
            };
            let cache_ref = cache_store
                .as_ref()
                .map(|c| c as &dyn llmsort::cache::PairwiseCache);

            let (judgement, usage) = llmsort::rerank::compare_pair(
                &gateway,
                cache_ref,
                llmsort::rerank::PairwiseComparisonRequest {
                    spec,
                    cache_only: false,
                    attribution: Attribution::new("cardinal::judge"),
                    nonce: None,
                },
            )
            .await?;

            let cost_usd = usage.provider_cost_nanodollars as f64 / 1e9;
            match judgement {
                llmsort::rerank::PairwiseJudgement::Observation {
                    higher_ranked,
                    ratio,
                    confidence,
                } => {
                    let winner = match higher_ranked {
                        llmsort::rerank::HigherRanked::A => "A",
                        llmsort::rerank::HigherRanked::B => "B",
                    };
                    if json {
                        println!(
                            "{}",
                            serde_json::json!({
                                "higher_ranked": winner,
                                "ratio": ratio,
                                "confidence": confidence,
                                "refused": false,
                                "model": model,
                                "input_tokens": usage.input_tokens,
                                "output_tokens": usage.output_tokens,
                                "cost_nanodollars": usage.provider_cost_nanodollars,
                                "cached": usage.cached,
                            })
                        );
                    } else {
                        let cached = if usage.cached { " · cached" } else { "" };
                        println!(
                            "{winner} wins · ratio {ratio} · confidence {confidence:.2} · ${cost_usd:.4}{cached}"
                        );
                    }
                }
                llmsort::rerank::PairwiseJudgement::Refused => {
                    if json {
                        println!(
                            "{}",
                            serde_json::json!({
                                "refused": true,
                                "model": model,
                                "cost_nanodollars": usage.provider_cost_nanodollars,
                                "cached": usage.cached,
                            })
                        );
                    } else {
                        println!("REFUSED · ${cost_usd:.4}");
                    }
                }
            }
        }
        Commands::Elaborate { by, model } => {
            require_openrouter_key()?;
            let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
            let rubric = llmsort::rerank::elaborate_criterion(
                &gateway,
                model.as_deref(),
                &by,
                Attribution::new("cardinal::elaborate"),
            )
            .await?;
            println!("{}", rubric.elaborated);
            eprintln!(
                "elaborated \"{}\" via {} · {} in / {} out tokens · ${:.4}",
                rubric.original,
                rubric.model_used,
                rubric.input_tokens,
                rubric.output_tokens,
                rubric.provider_cost_nanodollars as f64 / 1e9,
            );
        }
        Commands::Explain {
            file,
            candidate,
            propose,
            model,
            budget,
            format_json,
            no_cache,
            cache,
            seed,
        } => {
            let raw = read_sort_input(file.as_deref())?;
            let documents = parse_sort_items(&raw)?;
            if documents.len() < 3 {
                return Err(
                    "explain requires at least 3 items (in your believed order, best first)".into(),
                );
            }
            require_openrouter_key()?;
            let gateway = Arc::new(ProviderGateway::from_env(Arc::new(NoopUsageSink))?);

            let mut candidates = candidate;
            if let Some(count) = propose {
                let (proposed, usage) = llmsort::rerank::propose_candidates(
                    gateway.as_ref(),
                    model.as_deref().unwrap_or("openai/gpt-5.4-mini"),
                    &documents,
                    count,
                    Attribution::new("cardinal::explain::propose"),
                )
                .await?;
                eprintln!(
                    "proposed {} candidate attributes (${:.4}):",
                    proposed.len(),
                    usage.cost_nanodollars as f64 / 1e9
                );
                for c in &proposed {
                    eprintln!("  - {c}");
                }
                candidates.extend(proposed);
            }
            if candidates.is_empty() {
                return Err(
                    "no candidate attributes: pass --candidate \"<attribute>\" (repeatable) and/or --propose <n>"
                        .into(),
                );
            }

            let cache_store = if no_cache {
                None
            } else {
                let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
                Some(SqlitePairwiseCache::new(cache_path)?)
            };
            let mut execution = llmsort::rerank::RerankExecution::new(
                gateway.clone(),
                Attribution::new("cardinal::explain"),
            )
            .run_options(RerankRunOptions {
                rng_seed: seed,
                cache_only: false,
            });
            if let Some(store) = cache_store.as_ref() {
                execution = execution.cache(store);
            }

            let explanation = llmsort::rerank::explain_ranking(
                documents,
                candidates,
                execution,
                llmsort::rerank::ExplainOptions {
                    model,
                    comparison_budget: budget,
                    ..Default::default()
                },
            )
            .await?;

            if format_json {
                println!("{}", serde_json::to_string_pretty(&explanation)?);
            } else {
                println!("attribute                                    | alone ρ | weight");
                println!("---------------------------------------------|---------|-------");
                for attr in &explanation.attributes {
                    let rho = attr
                        .spearman_alone
                        .map(|r| format!("{r:+.2}"))
                        .unwrap_or_else(|| "  n/a".into());
                    let prompt: String = attr.prompt.chars().take(44).collect();
                    println!("{prompt:<45}| {rho:>7} | {:.2}", attr.fitted_weight);
                }
                match explanation.combined_spearman {
                    Some(c) => println!(
                        "
weighted combination reconstructs your ranking at ρ = {c:+.2}"
                    ),
                    None => println!(
                        "
no combination of these attributes reconstructs your ranking"
                    ),
                }
            }
            let meta = &explanation.meta;
            eprintln!(
                "{} comparisons ({} cached, {} refused) · ${:.4} · order flips: {}/{}",
                meta.comparisons_used,
                meta.comparisons_cached,
                meta.comparisons_refused,
                meta.provider_cost_nanodollars as f64 / 1e9,
                meta.position_flips,
                meta.pairs_counterbalanced,
            );
        }
        Commands::CacheExport { db, out } => {
            let path = db.unwrap_or_else(SqlitePairwiseCache::default_path);
            let cache = SqlitePairwiseCache::new(path)?;
            cache.export_jsonl(out).await?;
        }
        Commands::CachePrune {
            db,
            max_age_days,
            max_rows,
        } => {
            if max_age_days.is_none() && max_rows.is_none() {
                return Err("cache-prune requires --max-age-days and/or --max-rows".into());
            }
            if matches!(max_rows, Some(0)) {
                return Err("--max-rows must be >= 1".into());
            }
            let path = db.unwrap_or_else(SqlitePairwiseCache::default_path);
            let cache = SqlitePairwiseCache::new(path)?;
            let _lock = cache.lock_exclusive()?;
            let stats = cache.prune(max_age_days, max_rows).await?;
            println!(
                "pruned {} rows; {} rows remain",
                stats.deleted, stats.remaining
            );
        }
        Commands::Policy { command } => match command {
            PolicyCommands::List => {
                let registry = PolicyRegistry::default();
                for name in registry.list() {
                    println!("{name}");
                }
            }
            PolicyCommands::Load { config } => {
                let policy = load_policy_from_path(config)?;
                let description = policy.describe().unwrap_or_else(|| "unknown".to_string());
                println!("{description}");
            }
        },
        Commands::Eval {
            case,
            out,
            curve_csv,
            mode,
        } => {
            let cfg = llmsort_experiments::evaluation::PairwiseEvalConfig { mode: mode.into() };
            let results = llmsort_experiments::evaluation::run_synthetic_suite_with_config(
                case.as_deref(),
                cfg,
            )?;
            let mut file = File::create(out)?;
            for result in &results {
                let line = serde_json::to_string(result)?;
                writeln!(file, "{line}")?;
            }
            if let Some(csv_path) = curve_csv {
                let mut csv = File::create(csv_path)?;
                writeln!(csv, "case,step,error")?;
                for result in results {
                    for (idx, err) in result.error_trajectory.iter().enumerate() {
                        writeln!(csv, "{},{},{}", result.case_name, idx, err)?;
                    }
                }
            }
        }
        Commands::EvalLikert {
            case,
            out,
            curve_csv,
            levels,
            budget_multiplier,
        } => {
            let cfg = llmsort_experiments::evaluation::LikertEvalConfig {
                levels,
                budget_multiplier,
            };
            let results =
                llmsort_experiments::evaluation::run_likert_baseline_suite(case.as_deref(), cfg)?;
            let mut file = File::create(out)?;
            for result in &results {
                let line = serde_json::to_string(result)?;
                writeln!(file, "{line}")?;
            }
            if let Some(csv_path) = curve_csv {
                let mut csv = File::create(csv_path)?;
                writeln!(csv, "case,step,error")?;
                for result in results {
                    for (idx, err) in result.error_trajectory.iter().enumerate() {
                        writeln!(csv, "{},{},{}", result.case_name, idx, err)?;
                    }
                }
            }
        }
        Commands::EvalCompare {
            case,
            out,
            levels,
            budget_multiplier,
            mode,
        } => {
            let pairwise_cfg =
                llmsort_experiments::evaluation::PairwiseEvalConfig { mode: mode.into() };
            let likert_cfg = llmsort_experiments::evaluation::LikertEvalConfig {
                levels,
                budget_multiplier,
            };
            let summary =
                llmsort_experiments::evaluation::run_evaluation_comparison_summary_with_config(
                    case.as_deref(),
                    pairwise_cfg,
                    likert_cfg,
                )?;
            let mut file = File::create(out)?;
            serde_json::to_writer_pretty(&mut file, &summary)?;
            writeln!(file)?;
        }
        Commands::Report {
            request,
            response,
            out,
            format,
            top_n,
            include_infeasible,
            no_attr_scores,
            rng_seed,
            policy,
            cache_only,
        } => {
            let req: MultiRerankRequest = read_json(&request)?;
            let resp: MultiRerankResponse = read_json(&response)?;
            validate_multi_rerank_request(&req)?;
            validate_report_inputs(&req, &resp)?;
            let opts = RerankReportOptions {
                top_n,
                include_infeasible,
                include_attribute_scores: !no_attr_scores,
                rng_seed,
                model_policy: policy,
                cache_only,
            };
            let report = build_report(&req, &resp, &opts);
            match format {
                ReportFormatArg::Json => {
                    let json = serde_json::to_string_pretty(&report)?;
                    std::fs::write(out, json)?;
                }
                ReportFormatArg::Md | ReportFormatArg::Markdown => {
                    let markdown = render_report_markdown(&report);
                    std::fs::write(out, markdown)?;
                }
            }
        }
        Commands::ExperimentExpand {
            request,
            out,
            prompt_template_slugs,
            include_negative,
            variant_json,
        } => {
            let req: MultiRerankRequest = read_json(&request)?;
            let mut variants = Vec::new();
            for path in variant_json {
                let mut loaded: Vec<AttributeVariantSpec> = read_json(&path)?;
                variants.append(&mut loaded);
            }
            let cfg = PromptExperimentConfig {
                prompt_template_slugs,
                include_negative,
                variants,
            };
            let expanded = expand_prompt_experiment_request(&req, &cfg)?;
            write_json(&out, &expanded)?;
            println!(
                "expanded request: {} attributes -> {} attributes",
                req.attributes.len(),
                expanded.attributes.len()
            );
        }
        Commands::Validate { request } => {
            let req: MultiRerankRequest = read_json(&request)?;
            validate_multi_rerank_request(&req)?;
            println!("valid request: {}", request.display());
        }
        Commands::Rerank {
            request,
            out,
            cache,
            lock_cache,
            cache_only,
            policy,
            policy_config,
            rng_seed,
            report,
            trace,
        } => {
            let req: MultiRerankRequest = read_json(&request)?;
            validate_multi_rerank_request(&req)?;
            let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
            let cache = SqlitePairwiseCache::new(cache_path)?;
            let _lock = if lock_cache {
                Some(cache.lock_exclusive()?)
            } else {
                None
            };

            let policy_obj = load_policy(policy, policy_config)?;
            let options = RerankRunOptions {
                rng_seed,
                cache_only,
            };
            let gateway = provider_gateway(cache_only)?;

            let (trace_sink, trace_worker) = if let Some(path) = trace {
                let (sink, worker) = JsonlTraceSink::new(path)?;
                (Some(sink), Some(worker))
            } else {
                (None, None)
            };
            let trace_ref = trace_sink.as_ref().map(|sink| sink as &dyn TraceSink);

            let mut execution = llmsort::rerank::RerankExecution::new(
                Arc::new(gateway),
                Attribution::new("cardinal::rerank"),
            )
            .cache(&cache)
            .run_options(options);
            if let Some(policy) = policy_obj.clone() {
                execution = execution.model_policy(policy);
            }
            if let Some(trace) = trace_ref {
                execution = execution.trace(trace);
            }

            let resp = llmsort::rerank::multi_rerank(req.clone(), execution).await?;

            write_json(&out, &resp)?;

            drop(trace_sink);
            if let Some(worker) = trace_worker {
                worker.join()?;
            }

            if let Some(report_path) = report {
                let opts = RerankReportOptions {
                    top_n: 10,
                    include_infeasible: false,
                    include_attribute_scores: true,
                    rng_seed,
                    model_policy: policy_obj.and_then(|policy| policy.describe()),
                    cache_only,
                };
                let report = build_report(&req, &resp, &opts);
                let markdown = render_report_markdown(&report);
                std::fs::write(report_path, markdown)?;
            }
        }
    }

    Ok(())
}

/// Resolve a judge item argument: literal text, or `@path` file contents.
fn read_item_arg(raw: &str) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(path) = raw.strip_prefix('@') {
        std::fs::read_to_string(path).map_err(|err| format!("failed to read {path}: {err}").into())
    } else {
        Ok(raw.to_string())
    }
}

/// Read raw sort input from a file or stdin (`-` or omitted).
fn read_sort_input(file: Option<&std::path::Path>) -> Result<String, Box<dyn std::error::Error>> {
    match file {
        Some(path) if path.as_os_str() != "-" => std::fs::read_to_string(path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()).into()),
        _ => {
            let mut raw = String::new();
            io::Read::read_to_string(&mut io::stdin(), &mut raw)?;
            Ok(raw)
        }
    }
}

/// Parse sort input: newline-delimited plain text, or a JSON array of strings
/// or `{"id", "text"}` objects when the first non-whitespace byte is `[`.
fn parse_sort_items(
    raw: &str,
) -> Result<Vec<llmsort::rerank::RerankDocument>, Box<dyn std::error::Error>> {
    use llmsort::rerank::RerankDocument;

    if raw.trim_start().starts_with('[') {
        let value: serde_json::Value = serde_json::from_str(raw)
            .map_err(|err| format!("input looks like JSON but failed to parse: {err}"))?;
        let arr = value
            .as_array()
            .ok_or("JSON input must be an array of strings or {id, text} objects")?;
        let mut documents = Vec::with_capacity(arr.len());
        for (idx, elem) in arr.iter().enumerate() {
            if let Some(text) = elem.as_str() {
                documents.push(RerankDocument {
                    id: format!("item-{idx:04}"),
                    text: text.to_string(),
                });
            } else if let Some(obj) = elem.as_object() {
                let text = obj
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| format!("JSON element {idx} needs a string \"text\" field"))?;
                let id = obj
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("item-{idx:04}"));
                documents.push(RerankDocument {
                    id,
                    text: text.to_string(),
                });
            } else {
                return Err(format!(
                    "JSON element {idx} must be a string or an object with a \"text\" field"
                )
                .into());
            }
        }
        Ok(documents)
    } else {
        Ok(raw
            .lines()
            .map(|line| line.strip_suffix('\r').unwrap_or(line))
            .filter(|line| !line.trim().is_empty())
            .enumerate()
            .map(|(idx, line)| RerankDocument {
                id: format!("item-{idx:04}"),
                text: line.to_string(),
            })
            .collect())
    }
}

/// Render sorted output in the requested format.
fn render_sorted(
    out: &mut impl Write,
    sorted: &llmsort::rerank::SortedTexts,
    format: SortFormatArg,
    scores: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        SortFormatArg::Text => {
            for item in &sorted.items {
                if scores {
                    writeln!(
                        out,
                        "{:.3}\u{b1}{:.3}\t{}",
                        item.latent_mean, item.latent_std, item.text
                    )?;
                } else {
                    writeln!(out, "{}", item.text)?;
                }
            }
        }
        SortFormatArg::Json => {
            serde_json::to_writer_pretty(&mut *out, sorted)?;
            writeln!(out)?;
        }
        SortFormatArg::Jsonl => {
            for item in &sorted.items {
                serde_json::to_writer(&mut *out, item)?;
                writeln!(out)?;
            }
        }
        SortFormatArg::Csv => {
            writeln!(
                out,
                "rank,id,latent_mean,latent_std,z_score,percentile,text"
            )?;
            for item in &sorted.items {
                writeln!(
                    out,
                    "{},{},{:.6},{:.6},{:.6},{:.6},{}",
                    item.rank,
                    csv_field(&item.id),
                    item.latent_mean,
                    item.latent_std,
                    item.z_score,
                    item.percentile,
                    csv_field(&item.text),
                )?;
            }
        }
    }
    Ok(())
}

/// Quote a CSV field when it contains a comma, quote, or newline.
fn csv_field(raw: &str) -> String {
    if raw.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", raw.replace('"', "\"\""))
    } else {
        raw.to_string()
    }
}

fn provider_gateway(
    cache_only: bool,
) -> Result<ProviderGateway<NoopUsageSink>, Box<dyn std::error::Error>> {
    if cache_only {
        let adapter = llmsort::gateway::openrouter::OpenRouterAdapter::with_config(
            "cache-only",
            "http://127.0.0.1:9",
            std::time::Duration::from_secs(1),
            None,
            None,
        )?;
        return Ok(ProviderGateway::with_config(
            adapter,
            Arc::new(NoopUsageSink),
            llmsort::gateway::GatewayConfig::default(),
        ));
    }

    if std::env::var("OPENROUTER_API_KEY").is_err() {
        return Err("OPENROUTER_API_KEY is not set. Create a key at \
             https://openrouter.ai/keys and `export OPENROUTER_API_KEY=...`, \
             or use --cache-only to replay cached judgements."
            .into());
    }

    Ok(ProviderGateway::from_env(Arc::new(NoopUsageSink))?)
}

fn require_openrouter_key() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("OPENROUTER_API_KEY").is_err() {
        return Err("OPENROUTER_API_KEY is not set. Create a key at \
             https://openrouter.ai/keys and `export OPENROUTER_API_KEY=...`."
            .into());
    }
    Ok(())
}

fn load_policy(
    policy: Option<String>,
    policy_config: Option<PathBuf>,
) -> Result<Option<Arc<dyn ModelPolicy>>, Box<dyn std::error::Error>> {
    if let Some(path) = policy_config {
        return Ok(Some(load_policy_from_path(path)?));
    }
    if let Some(name) = policy {
        let registry = PolicyRegistry::default();
        let available = registry.list().join(", ");
        let policy = registry
            .get(&name)
            .ok_or_else(|| format!("unknown policy '{name}'; available policies: {available}"))?;
        return Ok(Some(policy));
    }
    Ok(None)
}

fn read_json<T: serde::de::DeserializeOwned>(
    path: &PathBuf,
) -> Result<T, Box<dyn std::error::Error>> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read JSON from {}: {err}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|err| format!("failed to parse JSON in {}: {err}", path.display()).into())
}

fn write_json<T: serde::Serialize>(path: &PathBuf, value: &T) -> Result<(), io::Error> {
    let json = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    std::fs::write(path, json)
}

fn parse_likert_levels(raw: &str) -> Result<usize, String> {
    let value = raw
        .parse::<usize>()
        .map_err(|err| format!("invalid integer '{raw}': {err}"))?;
    if value >= 2 {
        Ok(value)
    } else {
        Err(format!("value must be at least 2, got {raw}"))
    }
}

fn parse_report_top_n(raw: &str) -> Result<usize, String> {
    let value = raw
        .parse::<usize>()
        .map_err(|err| format!("invalid integer '{raw}': {err}"))?;
    if value >= 1 {
        Ok(value)
    } else {
        Err(format!("value must be at least 1, got {raw}"))
    }
}

fn parse_positive_finite_f64(raw: &str) -> Result<f64, String> {
    let value = raw
        .parse::<f64>()
        .map_err(|err| format!("invalid number '{raw}': {err}"))?;
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(format!(
            "value must be a finite number greater than 0, got {raw}"
        ))
    }
}
