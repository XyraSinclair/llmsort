use super::*;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(super) enum ReportFormatArg {
    Md,
    Markdown,
    Json,
}

#[derive(Parser)]
#[command(
    name = "llmsort",
    version,
    about = "Canonical pairwise ratio CLI",
    after_help = "The stability-promised verbs are `sort` and `judge` (plus the judgment-packet \
format they emit). Verbs marked (research) are honest, provenanced instruments \
that are free to change shape without notice (AGENTS.md: canonical vs \
research-grade surface)."
)]
pub(super) struct Cli {
    #[command(subcommand)]
    pub(super) command: Commands,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(super) enum SortFormatArg {
    Text,
    Json,
    Jsonl,
    Csv,
}

#[derive(Subcommand)]
pub(super) enum Commands {
    /// Sort a list of items by a natural-language criterion
    ///
    /// Reads newline-delimited items (or a JSON array) from FILE or stdin and
    /// prints them sorted best-first. Requires OPENROUTER_API_KEY unless
    /// --cache-only is set and the cache already holds every judgement.
    ///
    /// Example: llmsort sort examples/sort-demo.txt --by "usefulness as advice"
    Sort {
        /// Input file; '-' or omitted reads stdin
        file: Option<PathBuf>,
        /// Criterion to sort by, e.g. "clarity of explanation"
        #[arg(long)]
        by: String,
        /// Model slug (OpenRouter), e.g. anthropic/claude-sonnet-4.6
        #[arg(long)]
        model: Option<String>,
        /// Built-in model policy name (see `llmsort policy list`)
        #[arg(long)]
        policy: Option<String>,
        /// Model policy JSON file
        #[arg(long)]
        policy_config: Option<PathBuf>,
        /// Maximum pairwise comparisons to spend
        #[arg(long)]
        budget: Option<usize>,
        /// Maximum provider-reported spend in dollars (pairwise only)
        #[arg(long)]
        max_dollars: Option<f64>,
        /// Maximum wall-clock runtime in seconds (pairwise only; checked
        /// between batches — may overshoot by one in-flight batch,
        /// including any rate-limit cooldown waits)
        #[arg(long)]
        max_seconds: Option<f64>,
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
        /// Use the setwise (k-at-a-time listwise) instrument instead of the
        /// pairwise path: ~1/4 the cost at adequate quality, order-sensitivity
        /// gauge printed on stderr. Supports --model/--k/--seed/--concurrency/
        /// --format/--scores/--reverse/--elaborate/--quiet only.
        #[arg(long)]
        setwise: bool,
        /// Setwise slots per call (measured band: 6-8)
        #[arg(long, default_value_t = 8)]
        k: usize,
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
        /// Prompt template. Default: auto — ratio_letter_2p_v1 (reasoned
        /// analysis, then a one-token PMF verdict via answer logprobs) for
        /// reasoning-class judges, ratio_letter_v1 (single-token PMF) for
        /// non-reasoning logprob models, canonical_v2 (JSON) elsewhere.
        /// Explicit: any of those, or canonical_bucket_v1 (evidence rails
        /// degrade loudly to sampled mode where providers hide logprobs)
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
pub(super) enum PolicyCommands {
    List,
    Load {
        #[arg(long)]
        config: PathBuf,
    },
}
