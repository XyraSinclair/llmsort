#![forbid(unsafe_code)]

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

mod cli;
mod helpers;
mod judge;
mod research;
mod sort;

use cli::{Cli, Commands, PolicyCommands, ReportFormatArg, SortFormatArg, SortInputArg};
use helpers::*;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cli = Cli::parse();

    let outcome = match cli.command {
        command @ Commands::Sort { .. } => sort::run(command).await,
        command @ Commands::Judge { .. } => judge::run(command).await,
        command @ (Commands::Explain { .. }
        | Commands::CacheExport { .. }
        | Commands::CachePrune { .. }
        | Commands::Policy { .. }
        | Commands::Report { .. }
        | Commands::Validate { .. }
        | Commands::Rerank { .. }) => research::run(command).await,
    };

    let Err(err) = outcome else {
        return std::process::ExitCode::SUCCESS;
    };
    eprintln!("error: {err}");
    let mut cause = err.source();
    while let Some(inner) = cause {
        eprintln!("  caused by: {inner}");
        cause = inner.source();
    }
    std::process::ExitCode::FAILURE
}
