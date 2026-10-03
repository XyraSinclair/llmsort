#!/usr/bin/env python3
"""Run a real OpenRouter-backed llmsorting benchmark suite.

This is intentionally not a synthetic evaluator. It shells out to the cardinal
CLI with OPENROUTER_API_KEY set, records provider-backed rerank studies, exports
cache rows, and writes a compact summary across runs.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class LiveCase:
    name: str
    description: str
    entities: list[dict[str, str]]
    attributes: list[dict[str, Any]]
    k: int
    tolerated_error: float
    comparison_budget: int
    comparison_concurrency: int


LIVE_CASES: tuple[LiveCase, ...] = (
    LiveCase(
        name="public_evidence_surfaces",
        description="Rank current llmsorting documentation surfaces by public-release evidence value.",
        entities=[
            {
                "id": "readme",
                "text": "README.md: public entry point. States llmsorting is a canonical list-sorting/reranking engine, explains the tradeoff versus scalar scoring, gives CLI examples, links algorithm, prompt, model, worked-example, evaluation, and benchmark docs.",
            },
            {
                "id": "evaluation_doc",
                "text": "research/notes/EVALUATION.md: checked-in evidence surface. Separates synthetic/offline studies from live LLM claims, lists reproducible commands, metric definitions, current cardinal-vs-Likert table, known gaps, and next empirical proof target.",
            },
            {
                "id": "prompts_doc",
                "text": "docs/PROMPTS.md: prompt contract. Documents canonical_v2 and canonical_bucket_v1 output schemas, ratio ladder, refusal shape, OpenRouter logprob constraints, model-policy recipes, and experiment expansion commands.",
            },
            {
                "id": "worked_example",
                "text": "docs/WORKED_EXAMPLE.md: concrete walkthrough. Shows request JSON, command lines, response interpretation, trace/report/cache evidence, replay, and what files to preserve for auditability.",
            },
            {
                "id": "model_doc",
                "text": "docs/MODEL.md: mathematical contract. Explains Bradley-Terry style latent score fitting, ratio observations, uncertainty, top-k stopping, gates, assumptions, and failure modes.",
            },
            {
                "id": "benchmarks_doc",
                "text": "docs/BENCHMARKS.md: scaling study. Reports local solver scaling, machine metadata, benchmark command, dense solver caveats, and what the numbers do and do not prove.",
            },
        ],
        attributes=[
            {
                "id": "public_readiness",
                "prompt": "public-readiness for a serious technical reader deciding whether this artifact is worth sharing widely",
                "prompt_template_slug": "canonical_v2",
                "weight": 0.4,
            },
            {
                "id": "evidence_value",
                "prompt": "strength of concrete, reproducible evidence rather than narrative assertion",
                "prompt_template_slug": "canonical_bucket_v1",
                "weight": 0.35,
            },
            {
                "id": "operator_reproducibility",
                "prompt": "ability for another engineer to rerun, audit, and falsify the claim from the documented commands and artifacts",
                "prompt_template_slug": "canonical_v2",
                "weight": 0.25,
            },
        ],
        k=3,
        tolerated_error=0.08,
        comparison_budget=72,
        comparison_concurrency=4,
    ),
    LiveCase(
        name="model_policy_live_routing",
        description="Rank candidate OpenRouter routing policies for a real public benchmark run.",
        entities=[
            {
                "id": "quality_only_opus_46",
                "text": "Fixed policy: anthropic/claude-opus-4.6. Highest-cost, high-quality judge path for public studies where answer quality and careful comparative reasoning matter more than cost.",
            },
            {
                "id": "frontier_ladder_2026_06",
                "text": "Ladder policy: starts anthropic/claude-opus-4.6, can step through google/gemini-3.1-pro-preview, then openai/gpt-5.4-mini as uncertainty falls. Intended to preserve quality on hard comparisons while reducing spend on easier or lower-impact comparisons.",
            },
            {
                "id": "cost_aware_deepseek_v4_flash",
                "text": "Fixed policy: deepseek/deepseek-v4-flash. Very low-cost current-generation route for high-volume sweeps, smoke tests, and prompt-surface iteration where exhaustive expensive judging would be wasteful.",
            },
            {
                "id": "qwen_3_7_max",
                "text": "Direct current OpenRouter model candidate: qwen/qwen3.7-max. Large context, current high-capability model with logprob support listed in live OpenRouter metadata.",
            },
            {
                "id": "glm_5_2",
                "text": "Direct current OpenRouter model candidate: z-ai/glm-5.2. Large context, very low prompt cost, supports logprobs and structured outputs in live OpenRouter metadata.",
            },
            {
                "id": "kimi_k2_thinking",
                "text": "Direct current OpenRouter model candidate: moonshotai/kimi-k2-thinking. Thinking-oriented current model, moderate context, useful as a diverse reasoning judge candidate.",
            },
        ],
        attributes=[
            {
                "id": "public_benchmark_reliability",
                "prompt": "reliability as a judge for public benchmark studies where brittle or unserious judgments would damage credibility",
                "prompt_template_slug": "canonical_v2",
                "weight": 0.45,
            },
            {
                "id": "cost_discipline",
                "prompt": "cost discipline for repeated real OpenRouter benchmark runs without sacrificing the validity of the evidence",
                "prompt_template_slug": "canonical_bucket_v1",
                "weight": 0.3,
            },
            {
                "id": "freshness_and_capability",
                "prompt": "current-generation capability and parameter support fit for cardinal pairwise ratio judging on OpenRouter",
                "prompt_template_slug": "canonical_v2",
                "weight": 0.25,
            },
        ],
        k=2,
        tolerated_error=0.08,
        comparison_budget=72,
        comparison_concurrency=4,
    ),
    LiveCase(
        name="public_release_risks",
        description="Rank remaining public-release risks by credibility impact and actionability.",
        entities=[
            {
                "id": "no_large_live_receipt",
                "text": "Risk: the repo has strong synthetic/offline studies but still needs a larger preserved live-LLM benchmark pack across task families, models, costs, and traces before making broad empirical claims.",
            },
            {
                "id": "dependency_audit_warnings",
                "text": "Risk: cargo audit currently exits with allowed transitive warnings for paste, anyhow, and rand. These are not necessarily exploitable here, but public release readers will notice them.",
            },
            {
                "id": "provider_metadata_drift",
                "text": "Risk: OpenRouter model IDs, pricing, supported parameters, and logprob behavior change over time. Checked-in policy recipes can become stale unless refreshed and dated.",
            },
            {
                "id": "cost_estimate_semantics",
                "text": "Risk: provider_cost_nanodollars is exact only when OpenRouter reports cost or local pricing table has an exact entry. Fallback estimates must stay labeled or readers may overtrust cost comparisons.",
            },
            {
                "id": "cache_provenance",
                "text": "Risk: cached comparisons improve reproducibility and cost, but a public study must make cache hits, model IDs, prompt hashes, and request hashes obvious enough to audit.",
            },
            {
                "id": "baseline_breadth",
                "text": "Risk: Likert and ordinal controls are present locally, but the live evidence should compare multiple real prompt families and budget policies rather than one weak baseline.",
            },
            {
                "id": "api_surface_complexity",
                "text": "Risk: the crate exposes advanced reranking, trace, policy, cache, gateway, and evaluation surfaces. A first-time public user may struggle unless the core path stays visibly small.",
            },
        ],
        attributes=[
            {
                "id": "credibility_risk",
                "prompt": "risk to public credibility if llmsorting is shared widely as a serious empirical artifact",
                "prompt_template_slug": "canonical_v2",
                "weight": 0.45,
            },
            {
                "id": "falsifiability_gap",
                "prompt": "how much this issue blocks concrete falsifiable live evidence rather than merely improving polish",
                "prompt_template_slug": "canonical_bucket_v1",
                "weight": 0.35,
            },
            {
                "id": "fix_leverage",
                "prompt": "leverage of fixing this issue for making the project safer to share widely",
                "prompt_template_slug": "canonical_v2",
                "weight": 0.2,
            },
        ],
        k=3,
        tolerated_error=0.08,
        comparison_budget=84,
        comparison_concurrency=4,
    ),
)


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, 1):
            if not line.strip():
                continue
            row = json.loads(line)
            if not isinstance(row, dict):
                raise SystemExit(f"{path}:{line_no}: expected JSON object")
            rows.append(row)
    return rows


def case_request(case: LiveCase) -> dict[str, Any]:
    return {
        "entities": case.entities,
        "attributes": case.attributes,
        "topk": {"k": case.k, "tolerated_error": case.tolerated_error},
        "gates": [],
        "comparison_budget": case.comparison_budget,
        "rater_id": f"live-openrouter-{case.name}",
        "comparison_concurrency": case.comparison_concurrency,
        "max_pair_repeats": 1,
        "randomize_presentation_order": True,
    }


def policy_name(policy_config: Path) -> str:
    data = read_json(policy_config)
    name = data.get("name")
    if not isinstance(name, str) or not name:
        raise SystemExit(f"{policy_config}: missing string policy name")
    return name


def run_command(cmd: list[str], cwd: Path) -> None:
    proc = subprocess.run(
        cmd,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        env=os.environ.copy(),
    )
    if proc.returncode != 0:
        print(proc.stdout)
        raise SystemExit(proc.returncode)
    if proc.stdout.strip():
        print(proc.stdout, end="" if proc.stdout.endswith("\n") else "\n")


def summarize_case(case: LiveCase, case_dir: Path, policy: str) -> dict[str, Any]:
    response = read_json(case_dir / "response.json")
    trace_rows = read_jsonl(case_dir / "trace.jsonl")
    cache_rows = read_jsonl(case_dir / "cache-export.jsonl")
    meta = response["meta"]
    top_entities = [
        {
            "id": entity["id"],
            "rank": entity["rank"],
            "u_mean": entity["u_mean"],
            "u_std": entity["u_std"],
            "p_flip": entity["p_flip"],
        }
        for entity in response["entities"][: case.k]
    ]
    trace_models = sorted({row["model"] for row in trace_rows})
    trace_cost = sum(int(row.get("provider_cost_nanodollars", 0)) for row in trace_rows)
    estimate_rows = sum(1 for row in trace_rows if row.get("provider_cost_is_estimate"))
    return {
        "case": case.name,
        "description": case.description,
        "policy": policy,
        "request": str(case_dir / "request.json"),
        "response": str(case_dir / "response.json"),
        "trace": str(case_dir / "trace.jsonl"),
        "report": str(case_dir / "report.md"),
        "cache_export": str(case_dir / "cache-export.jsonl"),
        "model_used": meta["model_used"],
        "trace_models": trace_models,
        "stop_reason": meta["stop_reason"],
        "global_topk_error": meta["global_topk_error"],
        "comparisons_attempted": meta["comparisons_attempted"],
        "comparisons_used": meta["comparisons_used"],
        "comparisons_cached": meta["comparisons_cached"],
        "comparisons_refused": meta["comparisons_refused"],
        "comparison_budget": meta["comparison_budget"],
        "provider_input_tokens": meta["provider_input_tokens"],
        "provider_output_tokens": meta["provider_output_tokens"],
        "provider_cost_nanodollars": meta["provider_cost_nanodollars"],
        "provider_cost_usd": meta["provider_cost_nanodollars"] / 1_000_000_000,
        "provider_cost_is_estimate": meta["provider_cost_is_estimate"],
        "trace_rows": len(trace_rows),
        "trace_cost_nanodollars": trace_cost,
        "trace_cost_usd": trace_cost / 1_000_000_000,
        "trace_cost_estimate_rows": estimate_rows,
        "cache_rows": len(cache_rows),
        "top_entities": top_entities,
    }


def write_markdown_summary(path: Path, summary: dict[str, Any]) -> None:
    lines = [
        "# Live OpenRouter Benchmark Study",
        "",
        "This study record is generated from real OpenRouter calls through `cardinal rerank`.",
        "It is not produced by the synthetic evaluator.",
        "",
        f"Policy: `{summary['policy']}`",
        f"Run directory: `{summary['out_dir']}`",
        f"Cases: {summary['case_count']}",
        f"Comparisons used: {summary['totals']['comparisons_used']}",
        f"Cached comparisons: {summary['totals']['comparisons_cached']}",
        f"Refusals: {summary['totals']['comparisons_refused']}",
        f"Provider input tokens: {summary['totals']['provider_input_tokens']}",
        f"Provider output tokens: {summary['totals']['provider_output_tokens']}",
        f"Provider cost: ${summary['totals']['provider_cost_usd']:.6f}",
        f"Trace cost estimate rows: {summary['totals']['trace_cost_estimate_rows']}",
        "",
        "| Case | Model used | Stop | Used/Budget | Cached | Refused | Cost USD | Top entities |",
        "|---|---|---|---:|---:|---:|---:|---|",
    ]
    for case in summary["cases"]:
        top = ", ".join(entity["id"] for entity in case["top_entities"])
        lines.append(
            "| "
            + " | ".join(
                [
                    f"`{case['case']}`",
                    f"`{case['model_used']}`",
                    f"`{case['stop_reason']}`",
                    f"{case['comparisons_used']}/{case['comparison_budget']}",
                    str(case["comparisons_cached"]),
                    str(case["comparisons_refused"]),
                    f"${case['provider_cost_usd']:.6f}",
                    top,
                ]
            )
            + " |"
        )
    lines.extend(
        [
            "",
            "## Interpretation guardrails",
            "",
            "- `comparisons_cached = 0` means this run made fresh provider calls for every comparison in the study.",
            "- A budget-exhausted stop is still a valid study result; it means the run spent the configured live-call budget before proving the tolerated top-k error bound.",
            "- This suite tests real provider integration, parsing, trace/cost accounting, and project-relevant ranking surfaces. It does not by itself prove global superiority over every scalar or ordinal baseline.",
            "",
        ]
    )
    path.write_text("\n".join(lines), encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser(description="Run real OpenRouter-backed cardinal benchmark studies.")
    parser.add_argument("--out-dir", required=True, type=Path)
    parser.add_argument("--policy-config", required=True, type=Path)
    parser.add_argument("--case", action="append", choices=[case.name for case in LIVE_CASES])
    parser.add_argument("--reuse-cache", action="store_true")
    parser.add_argument("--rng-seed", type=int, default=20260630)
    args = parser.parse_args()

    if not os.environ.get("OPENROUTER_API_KEY"):
        raise SystemExit("OPENROUTER_API_KEY is required for live OpenRouter benchmark runs")

    repo = Path(__file__).resolve().parents[1]
    policy = policy_name(args.policy_config)
    selected = [case for case in LIVE_CASES if args.case is None or case.name in set(args.case)]
    if not selected:
        raise SystemExit("no live benchmark cases selected")

    args.out_dir.mkdir(parents=True, exist_ok=True)
    case_summaries: list[dict[str, Any]] = []

    for case in selected:
        case_dir = args.out_dir / case.name
        case_dir.mkdir(parents=True, exist_ok=True)
        request_path = case_dir / "request.json"
        response_path = case_dir / "response.json"
        trace_path = case_dir / "trace.jsonl"
        report_path = case_dir / "report.md"
        cache_path = case_dir / "cache.sqlite"
        cache_export_path = case_dir / "cache-export.jsonl"
        lock_path = cache_path.with_suffix(".lock")

        if not args.reuse_cache:
            for path in (cache_path, lock_path, response_path, trace_path, report_path, cache_export_path):
                if path.exists():
                    path.unlink()

        write_json(request_path, case_request(case))
        print(f"running {case.name} with {policy}")
        run_command(
            [
                "cargo",
                "run",
                "--quiet",
                "--bin",
                "cardinal",
                "--",
                "rerank",
                "--request",
                str(request_path),
                "--cache",
                str(cache_path),
                "--policy-config",
                str(args.policy_config),
                "--out",
                str(response_path),
                "--trace",
                str(trace_path),
                "--report",
                str(report_path),
                "--rng-seed",
                str(args.rng_seed),
            ],
            repo,
        )
        run_command(
            [
                "cargo",
                "run",
                "--quiet",
                "--bin",
                "cardinal",
                "--",
                "cache-export",
                "--db",
                str(cache_path),
                "--out",
                str(cache_export_path),
            ],
            repo,
        )
        case_summaries.append(summarize_case(case, case_dir, policy))

    totals = {
        "comparisons_used": sum(case["comparisons_used"] for case in case_summaries),
        "comparisons_cached": sum(case["comparisons_cached"] for case in case_summaries),
        "comparisons_refused": sum(case["comparisons_refused"] for case in case_summaries),
        "provider_input_tokens": sum(case["provider_input_tokens"] for case in case_summaries),
        "provider_output_tokens": sum(case["provider_output_tokens"] for case in case_summaries),
        "provider_cost_nanodollars": sum(case["provider_cost_nanodollars"] for case in case_summaries),
        "provider_cost_usd": sum(case["provider_cost_usd"] for case in case_summaries),
        "trace_rows": sum(case["trace_rows"] for case in case_summaries),
        "trace_cost_estimate_rows": sum(case["trace_cost_estimate_rows"] for case in case_summaries),
    }
    summary = {
        "policy": policy,
        "policy_config": str(args.policy_config),
        "out_dir": str(args.out_dir),
        "case_count": len(case_summaries),
        "totals": totals,
        "cases": case_summaries,
    }
    write_json(args.out_dir / "summary.json", summary)
    write_markdown_summary(args.out_dir / "summary.md", summary)
    print(f"wrote {args.out_dir / 'summary.json'}")
    print(f"wrote {args.out_dir / 'summary.md'}")


if __name__ == "__main__":
    main()
