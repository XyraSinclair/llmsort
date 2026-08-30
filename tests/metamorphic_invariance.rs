//! Metamorphic invariances of the full sort path.
//!
//! Each test states a mathematical claim about `sort_texts`/`sort_documents`
//! and attacks it with a deterministic, content-addressed judge running
//! behind a wiremock double of the OpenRouter chat-completions endpoint (the
//! pattern from `tests/sort_cli.rs`). The judge reads the `<entity_A_context>`
//! / `<entity_B_context>` blocks that the prompt templates embed verbatim and
//! decides purely from a count of the marker character `'Z'` in the text —
//! it never looks at labels, ids, or presentation order, so any invariance
//! violation we observe is a property of the harness (planner, solver,
//! aggregation), not an artifact of a cheating judge.
//!
//! All tests use fixed seeds via `RerankRunOptions::rng_seed` and no
//! `.cache(..)` call on `RerankExecution` (the library equivalent of
//! `cardinal sort --no-cache --seed <n>`), so every run is fully
//! reproducible.

#![allow(clippy::await_holding_lock)] // deliberate: tests serialize via a suite permit

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use llmsort::gateway::openrouter::OpenRouterAdapter;
use llmsort::gateway::{Attribution, ChatGateway, GatewayConfig, NoopUsageSink, ProviderGateway};
use llmsort::rerank::{
    sort_documents, sort_texts, RerankDocument, RerankRunOptions, SortOptions, SortedTexts,
};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

// ---------------------------------------------------------------------------
// Deterministic, content-addressed judges
// ---------------------------------------------------------------------------

fn extract_between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let start_idx = s.find(start)? + start.len();
    let rest = &s[start_idx..];
    let end_idx = rest.find(end)?;
    Some(&rest[..end_idx])
}

/// The judge's entire notion of "how much of the attribute" an entity has:
/// the number of `'Z'` characters in its text. Purely a function of content,
/// so it cannot be fooled by which side ("A"/"B") an entity is presented on,
/// nor by what id it was given.
fn marks(text: &str) -> i32 {
    text.chars().filter(|c| *c == 'Z').count() as i32
}

fn ratio_for_gap(gap: i32) -> f64 {
    if gap >= 5 {
        3.7
    } else if gap >= 2 {
        2.1
    } else {
        1.3
    }
}

fn user_content(request: &Request) -> String {
    let parsed: serde_json::Value = serde_json::from_slice(&request.body).unwrap_or_default();
    parsed
        .get("messages")
        .and_then(|m| m.as_array())
        .and_then(|messages| {
            messages
                .iter()
                .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
                .and_then(|m| m.get("content").and_then(|c| c.as_str()))
                .map(str::to_string)
        })
        .unwrap_or_default()
}

fn judgement_body(higher: &str, ratio: f64) -> serde_json::Value {
    let content = format!(r#"{{"higher_ranked":"{higher}","ratio":{ratio},"confidence":0.9}}"#);
    json!({
        "choices": [{
            "message": { "content": content },
            "finish_reason": "stop"
        }],
        "usage": { "prompt_tokens": 10, "completion_tokens": 10 }
    })
}

/// Judges strictly by mark count: more `'Z'`s wins. Content-only, so it is
/// invariant to which entity is labeled A vs B and to entity ids.
#[derive(Clone, Copy)]
struct MarkJudge;

impl Respond for MarkJudge {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let content = user_content(request);
        let a = extract_between(&content, "<entity_A_context>", "</entity_A_context>")
            .map(marks)
            .unwrap_or(0);
        let b = extract_between(&content, "<entity_B_context>", "</entity_B_context>")
            .map(marks)
            .unwrap_or(0);
        let (higher, ratio) = if a >= b {
            ("A", ratio_for_gap(a - b))
        } else {
            ("B", ratio_for_gap(b - a))
        };
        ResponseTemplate::new(200).set_body_json(judgement_body(higher, ratio))
    }
}

/// Polarity-aware mark judge: inverts its preference whenever the attribute
/// prompt embedded in the user message contains "lack of". This models a
/// judge that genuinely understands negated criteria, which is what
/// `REVERSED-CRITERION ANTISYMMETRY` requires to be a meaningful test (a
/// judge that ignored polarity would trivially fail to reverse anything).
#[derive(Clone, Copy)]
struct PolarityMarkJudge;

impl Respond for PolarityMarkJudge {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let content = user_content(request);
        let inverted = content.contains("lack of");
        let a = extract_between(&content, "<entity_A_context>", "</entity_A_context>")
            .map(marks)
            .unwrap_or(0);
        let b = extract_between(&content, "<entity_B_context>", "</entity_B_context>")
            .map(marks)
            .unwrap_or(0);
        let (a_eff, b_eff) = if inverted { (-a, -b) } else { (a, b) };
        let (higher, ratio) = if a_eff >= b_eff {
            ("A", ratio_for_gap(a_eff - b_eff))
        } else {
            ("B", ratio_for_gap(b_eff - a_eff))
        };
        ResponseTemplate::new(200).set_body_json(judgement_body(higher, ratio))
    }
}

async fn start_judge<R: Respond + 'static>(judge: R) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(judge)
        .mount(&server)
        .await;
    server
}

// The whole suite runs in well under a second single-threaded (see
// `suite_permit` below), so a generous per-request timeout costs nothing on
// the happy path. It matters under `cargo test`'s default *cross-test*
// thread parallelism: with 11 tests each driving their own tokio runtime and
// synchronous IRLS solves, a resource-constrained sandbox can starve the OS
// thread waiting on a same-machine wiremock response past a tight timeout.
// A too-tight client timeout on a same-machine mock is a test-harness
// availability failure, not a signal about the judge or the solver -- it
// must not masquerade as a metamorphic-invariance violation.
const JUDGE_TIMEOUT: Duration = Duration::from_secs(30);

/// Serializes test bodies within this binary. Every test here is already
/// deterministic given (seed, judge, inputs); this lock exists purely to
/// remove *inter-test* resource contention (CPU for concurrent IRLS solves,
/// OS scheduling latency for concurrent localhost HTTP) as a confound, so a
/// real order-invariance/reversal/etc. violation can never be masked by --
/// or mistaken for -- a starved mock request. Total serialized runtime for
/// all 11 tests is well under a second, so this buys determinism for free.
static SUITE_LOCK: Mutex<()> = Mutex::new(());

fn suite_permit() -> std::sync::MutexGuard<'static, ()> {
    SUITE_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

fn gateway_for(server: &MockServer) -> Arc<dyn ChatGateway> {
    let adapter =
        OpenRouterAdapter::with_config("sk-test", server.uri(), JUDGE_TIMEOUT, None, None).unwrap();
    Arc::new(ProviderGateway::with_config(
        adapter,
        Arc::new(NoopUsageSink),
        GatewayConfig {
            max_retries: 0,
            retry_base_delay: Duration::from_millis(0),
        },
    ))
}

fn execution_for(
    server: &MockServer,
    seed: u64,
    tag: &'static str,
) -> llmsort::rerank::RerankExecution<'static> {
    llmsort::rerank::RerankExecution::new(gateway_for(server), Attribution::new(tag)).run_options(
        RerankRunOptions {
            rng_seed: Some(seed),
            cache_only: false,
        },
    )
}

/// How many times to redo a whole sort call if the transport dropped some
/// comparisons outright (`comparisons_used < comparisons_attempted` with
/// zero refusals -- see `all_comparisons_landed` below).
const TRANSPORT_RETRY_LIMIT: u32 = 14;

/// `comparisons_used` counts "comparisons that produced observations";
/// `comparisons_attempted` counts all of them, refusals included (see the
/// doc comments on `RerankMeta`/`MultiRerankMeta`). Our judges never refuse,
/// so on this deterministic content-only judge `used < attempted` can only
/// mean a comparison's HTTP round trip to the loopback wiremock server
/// itself failed -- a transport availability problem, not a planner/solver
/// outcome. On a heavily loaded, multi-tenant sandbox this can happen in a
/// burst (observed empirically: system-wide local ephemeral-port/CPU
/// pressure from unrelated concurrent processes causes a run of near-
/// instant connection failures lasting a couple of seconds, not a single
/// dropped request), so retrying immediately just re-loses; backing off
/// gives the transient exhaustion time to clear. Retrying the whole call is
/// what a production caller would do on total judge unavailability, and it
/// changes nothing about what the metamorphic assertions below measure
/// (they all operate on the returned order/means of a *fully served* run).
fn all_comparisons_landed(sorted: &SortedTexts) -> bool {
    sorted.meta.comparisons_used == sorted.meta.comparisons_attempted
}

async fn transport_backoff(attempt: u32) {
    let millis = 100u64.saturating_mul(1u64 << attempt.min(5));
    tokio::time::sleep(Duration::from_millis(millis.min(2_000))).await;
}

async fn run_sort(
    server: &MockServer,
    texts: Vec<String>,
    criterion: &str,
    opts: SortOptions,
    seed: u64,
    tag: &'static str,
) -> SortedTexts {
    let mut last = None;
    for attempt in 0..TRANSPORT_RETRY_LIMIT {
        if attempt > 0 {
            transport_backoff(attempt - 1).await;
        }
        let r = sort_texts(
            texts.clone(),
            criterion,
            execution_for(server, seed, tag),
            opts.clone(),
        )
        .await
        .expect("sort_texts should succeed against a deterministic mock judge");
        if all_comparisons_landed(&r) {
            return r;
        }
        last = Some(r);
    }
    last.expect("TRANSPORT_RETRY_LIMIT > 0")
}

async fn run_sort_documents(
    server: &MockServer,
    docs: Vec<RerankDocument>,
    criterion: &str,
    opts: SortOptions,
    seed: u64,
    tag: &'static str,
) -> SortedTexts {
    let mut last = None;
    for attempt in 0..TRANSPORT_RETRY_LIMIT {
        if attempt > 0 {
            transport_backoff(attempt - 1).await;
        }
        let r = sort_documents(
            docs.clone(),
            criterion,
            execution_for(server, seed, tag),
            opts.clone(),
        )
        .await
        .expect("sort_documents should succeed against a deterministic mock judge");
        if all_comparisons_landed(&r) {
            return r;
        }
        last = Some(r);
    }
    last.expect("TRANSPORT_RETRY_LIMIT > 0")
}

fn texts_of(sorted: &SortedTexts) -> Vec<String> {
    sorted.items.iter().map(|i| i.text.clone()).collect()
}

fn budgeted(budget: usize) -> SortOptions {
    SortOptions {
        comparison_budget: Some(budget),
        // The mock judges answer canonical JSON; pin that instrument so the
        // per-model template default (which would route the default model to
        // the single-token PMF rail) never applies to a mocked judge.
        prompt_template_slug: Some("canonical_v2".into()),
        ..Default::default()
    }
}

// A clean, five-way separated ladder of mark counts. Every pairwise gap is
// >= 2 so the judge's decision is never a coin flip and the true order is
// unambiguous: ALPHA > BETA > GAMMA > DELTA (> EPSILON, added in the IIA test).
const ALPHA: &str = "alpha widget ZZZZZZZZZ"; // 9 marks
const BETA: &str = "beta widget ZZZZZZ"; // 6 marks
const GAMMA: &str = "gamma widget ZZZ"; // 3 marks
const DELTA: &str = "delta widget Z"; // 1 mark
const EPSILON: &str = "epsilon widget"; // 0 marks — clearly worst

fn ladder() -> Vec<String> {
    vec![ALPHA.into(), BETA.into(), GAMMA.into(), DELTA.into()]
}

fn true_order() -> Vec<String> {
    vec![ALPHA.into(), BETA.into(), GAMMA.into(), DELTA.into()]
}

// ---------------------------------------------------------------------------
// 1. INPUT-ORDER INVARIANCE
// ---------------------------------------------------------------------------
//
// Claim: for a fixed seed and fixed judge, sort_texts(perm(items)) yields the
// same output *text* ranking regardless of the order `items` was handed in.
// The planner may explore the comparison graph in a different sequence when
// the input order changes (its internal indices shift), but with a budget
// generous enough to reach the tolerated-error stopping criterion, the fitted
// order must converge to the same content-determined ranking.

#[tokio::test]
async fn input_order_invariance_holds_across_permutations_and_seeds() {
    let _guard = suite_permit();
    let server = start_judge(MarkJudge).await;
    let base = ladder();
    let permutations: Vec<Vec<String>> = vec![
        base.clone(),
        base.iter().rev().cloned().collect(),
        vec![
            base[2].clone(),
            base[0].clone(),
            base[3].clone(),
            base[1].clone(),
        ],
    ];

    let perm_tags: [&'static str; 3] = ["order-inv-perm-0", "order-inv-perm-1", "order-inv-perm-2"];
    for seed in 1u64..=8 {
        let mut orders = Vec::new();
        for (perm_idx, perm) in permutations.iter().enumerate() {
            let sorted = run_sort(
                &server,
                perm.clone(),
                "marks",
                budgeted(24),
                seed,
                perm_tags[perm_idx],
            )
            .await;
            orders.push(texts_of(&sorted));
        }
        for (idx, order) in orders.iter().enumerate() {
            assert_eq!(
                order, &orders[0],
                "seed {seed}: permutation {idx} produced a different output order than \
                 permutation 0 (order[0]={:?}, order[{idx}]={:?})",
                orders[0], order
            );
        }
        assert_eq!(
            orders[0],
            true_order(),
            "seed {seed}: the converged order must also match the content-determined truth"
        );
    }
}

#[tokio::test]
async fn input_order_invariance_survives_top_k_and_pruning() {
    let _guard = suite_permit();
    // Same claim, but exercised through the top-k + pruning code path
    // (top_k, prune_p_topk_below), which takes a materially different route
    // through the planner than a whole-list sort.
    let server = start_judge(MarkJudge).await;
    let base = ladder();
    let reversed: Vec<String> = base.iter().rev().cloned().collect();

    let opts = SortOptions {
        comparison_budget: Some(32),
        top_k: Some(2),
        prune_p_topk_below: Some(0.15),
        prompt_template_slug: Some("canonical_v2".into()),
        ..Default::default()
    };

    let forward = run_sort(&server, base, "marks", opts.clone(), 3, "topk-inv-fwd").await;
    let backward = run_sort(&server, reversed, "marks", opts, 3, "topk-inv-bwd").await;

    assert_eq!(
        texts_of(&forward),
        texts_of(&backward),
        "top-k/pruning path must also be input-order invariant"
    );
    // The certified top-2 boundary must match content, regardless of input order.
    assert_eq!(texts_of(&forward)[0], ALPHA);
    assert_eq!(texts_of(&forward)[1], BETA);
}

// ---------------------------------------------------------------------------
// 2. RELABELING INVARIANCE
// ---------------------------------------------------------------------------
//
// Claim: sort_documents' output *text* order depends only on the texts and
// their relative input positions, never on the id strings attached to them.

fn docs_with_ids(ids: &[&str]) -> Vec<RerankDocument> {
    let texts = ladder();
    ids.iter()
        .zip(texts)
        .map(|(id, text)| RerankDocument {
            id: (*id).to_string(),
            text,
        })
        .collect()
}

#[tokio::test]
async fn relabeling_invariance_ids_do_not_affect_text_order() {
    let _guard = suite_permit();
    let server = start_judge(MarkJudge).await;
    let seed = 11;

    let a = run_sort_documents(
        &server,
        docs_with_ids(&["doc-0", "doc-1", "doc-2", "doc-3"]),
        "marks",
        budgeted(24),
        seed,
        "relabel-a",
    )
    .await;
    let b = run_sort_documents(
        &server,
        docs_with_ids(&["zzzz-99", "aardvark", "mid-id", "q7"]),
        "marks",
        budgeted(24),
        seed,
        "relabel-b",
    )
    .await;

    assert_eq!(
        texts_of(&a),
        texts_of(&b),
        "relabeling document ids must not change the output text order"
    );
    assert_eq!(texts_of(&a), true_order());
}

#[tokio::test]
async fn relabeling_invariance_holds_across_seed_ensemble() {
    let _guard = suite_permit();
    let server = start_judge(MarkJudge).await;
    let id_sets: [[&str; 4]; 2] = [
        ["doc-0", "doc-1", "doc-2", "doc-3"],
        ["xk4", "9-alpha-id", "m", "the-quick-brown-fox"],
    ];

    let set_tags: [&'static str; 2] = ["relabel-ens-set-0", "relabel-ens-set-1"];
    for seed in 1u64..=8 {
        let mut orders = Vec::new();
        for (set_idx, ids) in id_sets.iter().enumerate() {
            let sorted = run_sort_documents(
                &server,
                docs_with_ids(ids),
                "marks",
                budgeted(24),
                seed,
                set_tags[set_idx],
            )
            .await;
            orders.push(texts_of(&sorted));
        }
        assert_eq!(
            orders[0], orders[1],
            "seed {seed}: two id-labelings of the same text sequence diverged"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. DUPLICATE CONSISTENCY
// ---------------------------------------------------------------------------
//
// Claim: two byte-identical texts under distinct ids are indistinguishable
// to a content-only judge, so the fitted model should treat them as (nearly)
// tied: adjacent in rank, or with a latent-mean gap small relative to the
// spread of the whole list. This is treated as a statistical claim (the
// planner's exact comparison sequence for the tied pair is seed-dependent)
// and is assessed over a seed ensemble rather than a single draw.

fn duplicate_docs() -> Vec<RerankDocument> {
    vec![
        RerankDocument {
            id: "alpha".into(),
            text: ALPHA.into(),
        },
        RerankDocument {
            id: "beta".into(),
            text: BETA.into(),
        },
        RerankDocument {
            id: "dup-1".into(),
            text: GAMMA.into(),
        },
        RerankDocument {
            id: "dup-2".into(),
            text: GAMMA.into(),
        },
        RerankDocument {
            id: "delta".into(),
            text: DELTA.into(),
        },
    ]
}

fn duplicate_holds(sorted: &SortedTexts) -> bool {
    let dup1 = sorted.items.iter().find(|i| i.id == "dup-1").unwrap();
    let dup2 = sorted.items.iter().find(|i| i.id == "dup-2").unwrap();
    let means: Vec<f64> = sorted.items.iter().map(|i| i.latent_mean).collect();
    let spread = means.iter().cloned().fold(f64::MIN, f64::max)
        - means.iter().cloned().fold(f64::MAX, f64::min);
    let mean_gap = (dup1.latent_mean - dup2.latent_mean).abs();
    let rank_gap = (dup1.rank as i64 - dup2.rank as i64).abs();
    rank_gap <= 1 || spread <= f64::EPSILON || mean_gap <= 0.2 * spread
}

#[tokio::test]
async fn duplicate_consistency_single_seed_sanity() {
    let _guard = suite_permit();
    let server = start_judge(MarkJudge).await;
    let sorted = run_sort_documents(
        &server,
        duplicate_docs(),
        "marks",
        budgeted(40),
        21,
        "dup-single",
    )
    .await;
    assert!(
        duplicate_holds(&sorted),
        "identical texts under distinct ids should end up adjacent or near-tied: {:?}",
        sorted
            .items
            .iter()
            .map(|i| (i.id.as_str(), i.rank, i.latent_mean))
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn duplicate_consistency_holds_across_seed_ensemble() {
    let _guard = suite_permit();
    let server = start_judge(MarkJudge).await;
    let n = 15u64;
    let mut satisfied = 0u64;
    for seed in 0..n {
        let sorted = run_sort_documents(
            &server,
            duplicate_docs(),
            "marks",
            budgeted(40),
            seed,
            "dup-ens",
        )
        .await;
        if duplicate_holds(&sorted) {
            satisfied += 1;
        }
    }
    let rate = satisfied as f64 / n as f64;
    assert!(
        rate >= 0.8,
        "expected duplicate adjacency/near-tie in >= 80% of {n} seeded replicas, got {rate:.2} ({satisfied}/{n})"
    );
}

// ---------------------------------------------------------------------------
// 4. IRRELEVANT-ALTERNATIVE STABILITY
// ---------------------------------------------------------------------------
//
// Weak claim (asserted): adding one item that the judge rates strictly worse
// than every existing item must not invert the *relative* order among the
// pre-existing top three.
//
// Strong claim (NOT a general guarantee): the pre-existing items' full
// relative order is unaffected by the addition. IRLS+Huber over a
// re-planned comparison graph is not a from-scratch independence-of-
// irrelevant-alternatives (IIA) system — adding an item changes which pairs
// get sampled, so in principle it can perturb rank margins even when it
// does not invert the top-3. We attacked this directly with ties (two
// items sharing a mark count, a genuine coin flip for the judge),
// deliberately tight comparison budgets, and top-k pruning enabled
// simultaneously, across a large seed sweep, trying to force a visible
// order change — and could not: the robust solver's aggregation turned out
// to be far more IIA-stable in practice than the algorithm's design
// guarantees on paper. That is a real, falsifiable finding (a plausible
// regression — e.g. an off-by-one in how pruning re-indexes entities after
// insertion, or a warm-start-adjacent bug that leaks new-item information
// into old-item priors — would show up here as a nonzero failure count),
// so we assert it as an ensemble claim, not a general theorem.

#[tokio::test]
async fn irrelevant_alternative_stability_top3_relative_order_preserved() {
    let _guard = suite_permit();
    let server = start_judge(MarkJudge).await;
    let seed = 42;

    let before = run_sort(&server, ladder(), "marks", budgeted(24), seed, "iia-before").await;
    let top3_before: Vec<String> = texts_of(&before).into_iter().take(3).collect();

    let mut with_worst = ladder();
    with_worst.push(EPSILON.into());
    let after = run_sort(
        &server,
        with_worst,
        "marks",
        budgeted(40),
        seed,
        "iia-after",
    )
    .await;
    let after_order = texts_of(&after);

    // The clearly-worst item must not intrude on the top 3.
    assert!(
        !after_order[..3].contains(&EPSILON.to_string()),
        "a clearly-worst item must not enter the top 3: {after_order:?}"
    );
    let top3_after: Vec<String> = after_order.into_iter().take(3).collect();
    assert_eq!(
        top3_before, top3_after,
        "adding one clearly-worst item must not reorder the pre-existing top 3"
    );
}

#[tokio::test]
async fn irrelevant_alternative_stability_strong_full_order_survives_adversarial_sweep() {
    let _guard = suite_permit();
    let server = start_judge(MarkJudge).await;
    // Two items tie on mark count (y/y2), the budget is far below what is
    // needed to counterbalance every pair, and top-k pruning is active, so
    // the planner's exploration allocation is maximally sensitive to which
    // items are present -- the most adversarial, cheapest-to-run setting we
    // found that could plausibly expose a global-IIA violation.
    let items: Vec<String> = vec![
        "w ZZZZZ".into(), // 5
        "x ZZZZ".into(),  // 4
        "y ZZZ".into(),   // 3 (tie)
        "y2 ZZZ".into(),  // 3 (tie)
        "z ZZ".into(),    // 2
        "q Z".into(),     // 1
    ];
    let worst = "r".to_string(); // 0 marks, clearly worst
    let opts = SortOptions {
        comparison_budget: Some(8),
        top_k: Some(2),
        prune_p_topk_below: Some(0.3),
        prompt_template_slug: Some("canonical_v2".into()),
        ..Default::default()
    };

    let n_seeds = 40u64;
    let mut failures: Vec<(u64, Vec<String>, Vec<String>)> = Vec::new();
    for seed in 0..n_seeds {
        let before = run_sort(
            &server,
            items.clone(),
            "marks",
            opts.clone(),
            seed,
            "iia-strong-before",
        )
        .await;
        let mut with_worst = items.clone();
        with_worst.push(worst.clone());
        let after = run_sort(
            &server,
            with_worst,
            "marks",
            opts.clone(),
            seed,
            "iia-strong-after",
        )
        .await;
        let before_ranked = texts_of(&before);
        let after_ranked: Vec<String> = texts_of(&after)
            .into_iter()
            .filter(|t| t != &worst)
            .collect();
        if before_ranked != after_ranked {
            failures.push((seed, before_ranked, after_ranked));
        }
    }

    assert!(
        failures.is_empty(),
        "strong IIA is not a general guarantee of this model, but it held on every \
         one of {n_seeds} adversarial seeds here; a regression would show up as a \
         nonzero failure count. Failures: {failures:#?}"
    );
}

// ---------------------------------------------------------------------------
// 5. REVERSED-CRITERION ANTISYMMETRY
// ---------------------------------------------------------------------------
//
// Claim: given a judge that is polarity-aware (inverts its preference when
// the attribute prompt contains "lack of"), sorting by "marks" and sorting
// by "lack of marks" over the same items and seed must produce exactly
// reversed output orders.

#[tokio::test]
async fn reversed_criterion_antisymmetry_exact_reversal() {
    let _guard = suite_permit();
    let server = start_judge(PolarityMarkJudge).await;
    let seed = 5;

    let positive = run_sort(&server, ladder(), "marks", budgeted(24), seed, "rev-pos").await;
    let negative = run_sort(
        &server,
        ladder(),
        "lack of marks",
        budgeted(24),
        seed,
        "rev-neg",
    )
    .await;

    let positive_order = texts_of(&positive);
    let mut expected_negative_order = positive_order.clone();
    expected_negative_order.reverse();

    assert_eq!(positive_order, true_order());
    assert_eq!(
        texts_of(&negative),
        expected_negative_order,
        "sorting by the negated criterion must produce the exact reverse order"
    );
}

#[tokio::test]
async fn reversed_criterion_antisymmetry_holds_across_seed_ensemble() {
    let _guard = suite_permit();
    let server = start_judge(PolarityMarkJudge).await;
    for seed in 1u64..=8 {
        let positive = run_sort(
            &server,
            ladder(),
            "marks",
            budgeted(24),
            seed,
            "rev-ens-pos",
        )
        .await;
        let negative = run_sort(
            &server,
            ladder(),
            "lack of marks",
            budgeted(24),
            seed,
            "rev-ens-neg",
        )
        .await;
        let mut expected = texts_of(&positive);
        expected.reverse();
        assert_eq!(
            texts_of(&negative),
            expected,
            "seed {seed}: positive/negative criterion orders were not exact reversals"
        );
    }
}

#[tokio::test]
async fn reversed_criterion_antisymmetry_holds_without_counterbalance() {
    let _guard = suite_permit();
    // Antisymmetry must hold even when counterbalancing is off: the content-
    // only judge has no position bias to cancel in the first place, so
    // disabling counterbalance should not be what makes this pass or fail.
    let server = start_judge(PolarityMarkJudge).await;
    let seed = 9;
    let opts = SortOptions {
        comparison_budget: Some(24),
        counterbalance: false,
        prompt_template_slug: Some("canonical_v2".into()),
        ..Default::default()
    };

    let positive = run_sort(&server, ladder(), "marks", opts.clone(), seed, "rev-nc-pos").await;
    let negative = run_sort(&server, ladder(), "lack of marks", opts, seed, "rev-nc-neg").await;

    let mut expected = texts_of(&positive);
    expected.reverse();
    assert_eq!(texts_of(&negative), expected);
}
