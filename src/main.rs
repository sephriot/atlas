mod cli;
mod client_context;
mod config;
mod context;
mod error;
mod locking;
mod models;
mod storage;
mod tools;

use clap::Parser;

use cli::{Commands, OutputFormat};
use context::validate_name;

use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "atlas")]
#[command(about = "Atlas - CLI knowledge and memory manager for coding agents")]
#[command(version)]
#[command(arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Storage path for knowledge atoms (default: ~/.atlas)
    #[arg(long, short = 's', global = true)]
    storage: Option<PathBuf>,

    /// Organization name (requires --project)
    #[arg(long, global = true, requires = "project")]
    org: Option<String>,

    /// Project name (requires --org)
    #[arg(long, global = true, requires = "org")]
    project: Option<String>,

    /// Project root directory for repo storage mode (where .atlas/ is created)
    #[arg(long, global = true)]
    project_root: Option<PathBuf>,

    /// Output format
    #[arg(long, short = 'f', global = true, default_value_t = OutputFormat::Yaml)]
    format: OutputFormat,

    /// Output as JSON instead of YAML (deprecated; use --format json)
    #[arg(long, global = true)]
    json: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Set storage path via env var if provided via CLI
    if let Some(ref path) = cli.storage {
        std::env::set_var("ATLAS_STORAGE", path);
    }

    // Set org/project context via env vars if provided via CLI
    if let (Some(ref org), Some(ref project)) = (&cli.org, &cli.project) {
        validate_name(org)?;
        validate_name(project)?;
        std::env::set_var("ATLAS_ORG", org);
        std::env::set_var("ATLAS_PROJECT", project);
    }

    // Set project root for repo storage mode
    if let Some(ref path) = cli.project_root {
        std::env::set_var("ATLAS_PROJECT_ROOT", path);
    }

    let format = if cli.json {
        OutputFormat::Json
    } else {
        cli.format
    };

    cli::run(cli.command, format)
}
