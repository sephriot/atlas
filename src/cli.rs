use std::fmt;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use crate::client_context::ClientContext;
use crate::context::{detect_context_full, require_explicit_write_context};
use crate::error::AtlasError;
use crate::models::{AtomType, Confidence};
use crate::tools::{
    create_atom, delete_atom, enable_local_storage, get_atom, get_context, link, list_atoms,
    list_projects, search, unlink, update_atom, AtomUpdateRequest, AtomWriteRequest,
    DeleteAtomRequest, EnableLocalStorageRequest, GetAtomRequest, LinkRequest, ListAtomsRequest,
    SearchRequest,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Yaml,
    Json,
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputFormat::Yaml => write!(f, "yaml"),
            OutputFormat::Json => write!(f, "json"),
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Search atoms by query
    #[command(alias = "find")]
    Search {
        /// Search query (use '-' for stdin, or pipe text with no query)
        query: Option<String>,

        /// Filter by atom types
        #[arg(long = "type", short = 't')]
        types: Vec<AtomType>,

        /// Filter by tags
        #[arg(long, short = 'T')]
        tag: Vec<String>,

        /// Filter by confidence level
        #[arg(long, short = 'c')]
        confidence: Option<Confidence>,

        /// Page number (1-indexed)
        #[arg(long, short = 'p', default_value = "1")]
        page: usize,

        /// Results per page
        #[arg(long, short = 'n', default_value = "20")]
        page_size: usize,

        /// Scope filter: org name or org/project path
        #[arg(long)]
        scope: Option<String>,

        /// Print only full atom IDs, one per line
        #[arg(long)]
        ids: bool,
    },

    /// Get one or more atoms by ID
    #[command(alias = "read")]
    Get {
        /// Atom IDs (org/project/id, project/id, bare id, '-' for stdin)
        ids: Vec<String>,
    },

    /// List atoms with optional filters
    #[command(alias = "list")]
    Atoms {
        /// Filter by atom types
        #[arg(long = "type", short = 't')]
        types: Vec<AtomType>,

        /// Filter by tags
        #[arg(long, short = 'T')]
        tag: Vec<String>,

        /// Filter by confidence level
        #[arg(long, short = 'c')]
        confidence: Option<Confidence>,

        /// Maximum number of results
        #[arg(long, short = 'l', default_value = "1000")]
        limit: usize,

        /// Scope filter: org name or org/project path
        #[arg(long)]
        scope: Option<String>,

        /// Print only full atom IDs, one per line
        #[arg(long)]
        ids: bool,
    },

    /// Create a new atom
    Create {
        #[command(flatten)]
        content: AtomWriteArgs,
    },

    /// Update an existing atom by ID
    Update {
        /// Atom ID to update (org/project/id, project/id, or bare id)
        id: String,

        #[command(flatten)]
        content: AtomUpdateArgs,
    },

    /// Delete an atom
    Delete {
        /// Atom ID (org/project/id, project/id, or bare id)
        id: String,

        /// Delete even when other atoms link to this one
        #[arg(long)]
        force: bool,
    },

    /// Create a directed link between atoms
    Link {
        /// Source atom ID
        source: String,

        /// Target atom ID
        target: String,
    },

    /// Remove a directed link between atoms
    Unlink {
        /// Source atom ID
        source: String,

        /// Target atom ID
        target: String,
    },

    /// List all projects
    Projects,

    /// Show detected project context
    Context,

    /// Print agent instructions for using the Atlas CLI
    Instructions {
        /// Target agent/client style
        #[arg(long, default_value_t = ClientContext::default())]
        client: ClientContext,
    },

    /// Enable local storage for a project
    EnableLocal {
        /// Directory where .atlas is created (default: current directory)
        #[arg(long)]
        root: Option<PathBuf>,
    },
}

#[derive(Args, Debug)]
pub struct AtomWriteArgs {
    /// Short descriptive title
    #[arg(long)]
    title: String,

    /// Type of knowledge
    #[arg(long = "type", short = 't')]
    atom_type: AtomType,

    /// Confidence level
    #[arg(long, short = 'c')]
    confidence: Confidence,

    /// Brief explanation (use '-' for stdin; omitted reads piped stdin)
    #[arg(long)]
    summary: Option<String>,

    /// Extended content (use '-' for stdin)
    #[arg(long)]
    details: Option<String>,

    /// Potential pitfalls (can specify multiple)
    #[arg(long)]
    pitfall: Vec<String>,

    /// Keywords for search (can specify multiple)
    #[arg(long, short = 'T')]
    tag: Vec<String>,

    /// References (can specify multiple)
    #[arg(long)]
    source: Vec<String>,
}

#[derive(Args, Debug)]
pub struct AtomUpdateArgs {
    #[arg(long)]
    title: Option<String>,

    #[arg(long = "type", short = 't')]
    atom_type: Option<AtomType>,

    #[arg(long, short = 'c')]
    confidence: Option<Confidence>,

    #[arg(long)]
    summary: Option<String>,

    #[arg(long)]
    details: Option<String>,

    #[arg(long)]
    pitfall: Vec<String>,

    #[arg(long, short = 'T')]
    tag: Vec<String>,

    #[arg(long)]
    source: Vec<String>,

    /// Field to clear (can specify multiple)
    #[arg(long)]
    clear: Vec<ClearField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ClearField {
    Details,
    Pitfalls,
    Tags,
    Sources,
}

/// Run a CLI command and print output.
pub fn run(cmd: Commands, format: OutputFormat) -> anyhow::Result<()> {
    match cmd {
        Commands::Search {
            query,
            types,
            tag,
            confidence,
            page,
            page_size,
            scope,
            ids,
        } => {
            let query = resolve_input(query)?;
            let req = SearchRequest {
                query: if query.is_empty() { None } else { Some(query) },
                types: if types.is_empty() { None } else { Some(types) },
                tags: if tag.is_empty() { None } else { Some(tag) },
                confidence,
                page: Some(page),
                page_size: Some(page_size),
                scope,
            };
            let results = search(req)?;
            if ids {
                print_lines(results.results.iter().map(|result| result.id.as_str()));
            } else {
                print_output(&results, format)?;
            }
        }
        Commands::Get { ids } => {
            let ids = resolve_ids(ids)?;
            if ids.len() == 1 {
                let atom = get_atom(GetAtomRequest { id: ids[0].clone() })?;
                print_output(&atom, format)?;
            } else {
                let atoms = ids
                    .into_iter()
                    .map(|id| get_atom(GetAtomRequest { id }))
                    .collect::<Result<Vec<_>, _>>()?;
                print_output(&atoms, format)?;
            }
        }
        Commands::Atoms {
            types,
            tag,
            confidence,
            limit,
            scope,
            ids,
        } => {
            let req = ListAtomsRequest {
                types: if types.is_empty() { None } else { Some(types) },
                tags: if tag.is_empty() { None } else { Some(tag) },
                confidence,
                limit: Some(limit),
                scope,
            };
            let results = list_atoms(req)?;
            if ids {
                print_lines(results.iter().map(|result| result.id.as_str()));
            } else {
                print_output(&results, format)?;
            }
        }
        Commands::Create { content } => {
            let req = atom_write_request_from_args(content)?;
            let result = create_atom(req)?;
            print_output(&result, format)?;
        }
        Commands::Update { id, content } => {
            let req = atom_update_request_from_args(content)?;
            let result = update_atom(id, req)?;
            print_output(&result, format)?;
        }
        Commands::Delete { id, force } => {
            let result = delete_atom(DeleteAtomRequest { id, force })?;
            print_output(&result, format)?;
        }
        Commands::Link { source, target } => {
            let result = link(LinkRequest { source, target })?;
            print_output(&result, format)?;
        }
        Commands::Unlink { source, target } => {
            let result = unlink(LinkRequest { source, target })?;
            print_output(&result, format)?;
        }
        Commands::Projects => {
            let results = list_projects()?;
            print_output(&results, format)?;
        }
        Commands::Context => {
            let info = get_context()?;
            print_output(&info, format)?;
        }
        Commands::Instructions { client } => {
            let instructions = client.instructions();
            print!("{}", instructions);
            if !instructions.ends_with('\n') {
                println!();
            }
        }
        Commands::EnableLocal { root } => {
            let context = require_explicit_write_context(detect_context_full()?)?;
            let result = enable_local_storage(EnableLocalStorageRequest {
                org: context.org,
                project: context.project,
                root,
            })?;
            print_output(&result, format)?;
        }
    }
    Ok(())
}

fn atom_update_request_from_args(args: AtomUpdateArgs) -> Result<AtomUpdateRequest, AtlasError> {
    if args.summary.as_deref() == Some("-") {
        return Err(AtlasError::Validation(
            "Use --details - for update stdin input; --summary - is not supported for patch updates."
                .to_string(),
        ));
    }

    validate_update_clear_conflicts(&args)?;
    let details = match args.details {
        Some(details) if details == "-" => Some(read_stdin()?),
        Some(details) => Some(details),
        None => None,
    };
    let clears = |field| args.clear.contains(&field);

    let request = AtomUpdateRequest {
        title: args.title,
        atom_type: args.atom_type,
        confidence: args.confidence,
        summary: args.summary,
        details,
        clear_details: clears(ClearField::Details),
        pitfalls: (!args.pitfall.is_empty()).then_some(args.pitfall),
        clear_pitfalls: clears(ClearField::Pitfalls),
        tags: (!args.tag.is_empty()).then_some(args.tag),
        clear_tags: clears(ClearField::Tags),
        sources: (!args.source.is_empty()).then_some(args.source),
        clear_sources: clears(ClearField::Sources),
    };

    if request.title.is_none()
        && request.atom_type.is_none()
        && request.confidence.is_none()
        && request.summary.is_none()
        && request.details.is_none()
        && !request.clear_details
        && request.pitfalls.is_none()
        && !request.clear_pitfalls
        && request.tags.is_none()
        && !request.clear_tags
        && request.sources.is_none()
        && !request.clear_sources
    {
        return Err(AtlasError::Validation(
            "Provide at least one field to update or clear.".to_string(),
        ));
    }

    Ok(request)
}

fn validate_update_clear_conflicts(args: &AtomUpdateArgs) -> Result<(), AtlasError> {
    let clear = |field| args.clear.contains(&field);
    let conflicts = [
        (
            clear(ClearField::Details) && args.details.is_some(),
            "details",
        ),
        (
            clear(ClearField::Pitfalls) && !args.pitfall.is_empty(),
            "pitfalls",
        ),
        (clear(ClearField::Tags) && !args.tag.is_empty(), "tags"),
        (
            clear(ClearField::Sources) && !args.source.is_empty(),
            "sources",
        ),
    ];

    if let Some((_, field)) = conflicts.into_iter().find(|(conflicts, _)| *conflicts) {
        return Err(AtlasError::Validation(format!(
            "Cannot set and clear {field} in one update."
        )));
    }

    Ok(())
}

fn atom_write_request_from_args(args: AtomWriteArgs) -> Result<AtomWriteRequest, AtlasError> {
    let (summary, details) = resolve_atom_write_text(args.summary, args.details)?;
    Ok(AtomWriteRequest {
        title: args.title,
        atom_type: args.atom_type,
        confidence: args.confidence,
        summary,
        details,
        pitfalls: if args.pitfall.is_empty() {
            None
        } else {
            Some(args.pitfall)
        },
        tags: if args.tag.is_empty() {
            None
        } else {
            Some(args.tag)
        },
        sources: if args.source.is_empty() {
            None
        } else {
            Some(args.source)
        },
        links: None,
    })
}

/// Resolve input: if arg is Some("-") or None and stdin is piped, read from stdin.
fn resolve_input(arg: Option<String>) -> Result<String, AtlasError> {
    match arg {
        Some(s) if s == "-" => read_stdin(),
        Some(s) => Ok(s),
        None => {
            if !io::stdin().is_terminal() {
                read_stdin()
            } else {
                Ok(String::new())
            }
        }
    }
}

fn resolve_ids(args: Vec<String>) -> Result<Vec<String>, AtlasError> {
    if args.is_empty() {
        if !io::stdin().is_terminal() {
            let input = read_stdin()?;
            return parse_ids(&input);
        }
        return Err(AtlasError::Validation(
            "At least one atom ID is required. Pass IDs as arguments, use '-', or pipe IDs on stdin."
                .to_string(),
        ));
    }

    if args.iter().any(|arg| arg == "-") {
        if args.len() != 1 {
            return Err(AtlasError::Validation(
                "'-' must be the only get argument when reading IDs from stdin".to_string(),
            ));
        }
        let input = read_stdin()?;
        return parse_ids(&input);
    }

    Ok(args)
}

fn parse_ids(input: &str) -> Result<Vec<String>, AtlasError> {
    let ids: Vec<String> = input
        .split_whitespace()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    if ids.is_empty() {
        return Err(AtlasError::Validation(
            "No atom IDs found in stdin".to_string(),
        ));
    }

    Ok(ids)
}

fn resolve_atom_write_text(
    summary: Option<String>,
    details: Option<String>,
) -> Result<(String, Option<String>), AtlasError> {
    let summary_from_stdin = summary.as_deref() == Some("-");
    let details_from_stdin = details.as_deref() == Some("-");
    let summary_missing = summary.is_none();

    let stdin = if summary_from_stdin
        || details_from_stdin
        || (summary_missing && !io::stdin().is_terminal())
    {
        Some(read_stdin()?)
    } else {
        None
    };

    resolve_atom_write_text_from_source(summary, details, stdin)
}

fn resolve_atom_write_text_from_source(
    summary: Option<String>,
    details: Option<String>,
    stdin: Option<String>,
) -> Result<(String, Option<String>), AtlasError> {
    let summary_from_stdin = summary.as_deref() == Some("-");
    let details_from_stdin = details.as_deref() == Some("-");

    if summary_from_stdin && details_from_stdin {
        return Err(AtlasError::Validation(
            "Only one of --summary or --details can read from stdin".to_string(),
        ));
    }

    let summary = match summary {
        Some(value) if value == "-" => stdin.clone().ok_or_else(stdin_required_error)?,
        Some(value) => value,
        None => stdin.clone().ok_or_else(summary_required_error)?,
    };

    if summary.trim().is_empty() {
        return Err(summary_required_error());
    }

    let details = match details {
        Some(value) if value == "-" => Some(stdin.ok_or_else(stdin_required_error)?),
        Some(value) => Some(value),
        None => None,
    }
    .filter(|value| !value.trim().is_empty());

    Ok((summary, details))
}

fn summary_required_error() -> AtlasError {
    AtlasError::Validation(
        "--summary is required unless summary text is piped on stdin".to_string(),
    )
}

fn stdin_required_error() -> AtlasError {
    AtlasError::Validation("Expected stdin for '-' but no piped input was available".to_string())
}

/// Read all input from stdin.
fn read_stdin() -> Result<String, AtlasError> {
    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .map_err(AtlasError::Io)?;
    Ok(buffer.trim().to_string())
}

fn print_lines<'a>(lines: impl Iterator<Item = &'a str>) {
    for line in lines {
        println!("{}", line);
    }
}

/// Print output as YAML (default) or JSON.
fn print_output<T: Serialize>(value: &T, format: OutputFormat) -> Result<(), AtlasError> {
    match format {
        OutputFormat::Json => {
            let output = serde_json::to_string_pretty(value)
                .map_err(|e| AtlasError::Config(format!("JSON serialization error: {}", e)))?;
            println!("{}", output);
        }
        OutputFormat::Yaml => {
            let output = serde_yaml::to_string(value)?;
            print!("{}", output);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_format_display_matches_cli_values() {
        assert_eq!(OutputFormat::Yaml.to_string(), "yaml");
        assert_eq!(OutputFormat::Json.to_string(), "json");
    }

    #[test]
    fn parse_ids_splits_whitespace() {
        let ids = parse_ids("K-000001\nproj/K-000002 org/proj/K-000003").expect("ids should parse");
        assert_eq!(
            ids,
            vec![
                "K-000001".to_string(),
                "proj/K-000002".to_string(),
                "org/proj/K-000003".to_string()
            ]
        );
    }

    #[test]
    fn atom_write_text_uses_piped_stdin_as_summary_when_summary_missing() {
        let (summary, details) =
            resolve_atom_write_text_from_source(None, None, Some("remember this".to_string()))
                .expect("stdin should become summary");
        assert_eq!(summary, "remember this");
        assert_eq!(details, None);
    }

    #[test]
    fn atom_write_text_ignores_implicit_stdin_when_summary_set() {
        let (summary, details) = resolve_atom_write_text_from_source(
            Some("short".to_string()),
            None,
            Some("longer markdown".to_string()),
        )
        .expect("provided summary should be enough");
        assert_eq!(summary, "short");
        assert_eq!(details, None);
    }

    #[test]
    fn atom_write_text_uses_explicit_stdin_as_details_when_requested() {
        let (summary, details) = resolve_atom_write_text_from_source(
            Some("short".to_string()),
            Some("-".to_string()),
            Some("longer markdown".to_string()),
        )
        .expect("explicit details stdin should be used");
        assert_eq!(summary, "short");
        assert_eq!(details, Some("longer markdown".to_string()));
    }

    #[test]
    fn atom_write_text_rejects_two_stdin_consumers() {
        let err = resolve_atom_write_text_from_source(
            Some("-".to_string()),
            Some("-".to_string()),
            Some("text".to_string()),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("Only one of --summary or --details"));
    }
}
