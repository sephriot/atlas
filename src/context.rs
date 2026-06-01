use std::path::{Path, PathBuf};
use std::process::Command;

use regex::Regex;
use serde::Serialize;

use crate::error::AtlasError;

/// Source of detected context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    /// Explicit configuration via ATLAS_ORG/ATLAS_PROJECT env vars or CLI flags
    EnvVars,
    /// Detected .atlas/ in current working directory
    LocalStorage,
    /// Automatic detection from git remote URL
    GitRemote,
    /// Fallback: global/{dirname}
    Fallback,
}

/// Detected context with source tracking.
#[derive(Debug, Clone)]
pub struct DetectedContext {
    pub context: ProjectContext,
    pub source: ContextSource,
}

impl DetectedContext {
    pub fn new(context: ProjectContext, source: ContextSource) -> Self {
        Self { context, source }
    }

    /// Returns true if context was detected via fallback.
    #[allow(dead_code)]
    pub fn is_fallback(&self) -> bool {
        matches!(self.source, ContextSource::Fallback)
    }
}

/// Detected project context.
#[derive(Debug, Clone)]
pub struct ProjectContext {
    pub org: String,
    pub project: String,
}

impl ProjectContext {
    pub fn new(org: String, project: String) -> Self {
        Self { org, project }
    }
}

/// Validate org/project name.
///
/// Names must:
/// - Start with alphanumeric character
/// - Contain only alphanumeric, hyphens, and underscores
/// - Not be empty
pub fn validate_name(name: &str) -> Result<(), AtlasError> {
    if name.is_empty() {
        return Err(AtlasError::Validation("Name cannot be empty".to_string()));
    }

    let re = Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9_-]*$").unwrap();
    if !re.is_match(name) {
        return Err(AtlasError::Validation(format!(
            "Invalid name '{}': must start with alphanumeric and contain only alphanumeric, hyphens, underscores",
            name
        )));
    }

    Ok(())
}

/// Full context detection with source tracking.
///
/// Detection follows a specific priority order to ensure the most explicit
/// and reliable context sources are used first:
///
/// 1. **Environment Variables / CLI Flags** (Highest Priority)
///    - Explicitly set via ATLAS_ORG/ATLAS_PROJECT env vars
///    - Also set by --org/--project CLI arguments
///    - Most reliable as they are explicitly configured by user
///
/// 2. **Local Storage** (.atlas/ in Current Working Directory)
///    - Looks for .atlas/config.yaml in current directory
///    - Useful for repository-local configuration
///    - Allows different contexts for different checkouts/workspaces
///
/// 3. **Git Remote URL Parsing**
///    - Extracts org/project from git remote get-url origin
///    - Supports common formats:
///      - git@github.com:org/project.git
///      - https://github.com/org/project.git
///      - git@gitlab.com:org/project.git
///      - https://gitlab.com/org/project.git
///    - Good default when no explicit configuration is provided
///
/// 4. **Fallback** (Lowest Priority)
///    - Uses global/{directory_name} as last resort
///    - Ensures we always return a valid context
///    - Less reliable but prevents complete failure
///
/// # Returns
/// * Result<DetectedContext, AtlasError> - The detected context with source tracking
pub fn detect_context_full() -> Result<DetectedContext, AtlasError> {
    // 1. Env vars / CLI flags
    if let (Ok(org), Ok(project)) = (std::env::var("ATLAS_ORG"), std::env::var("ATLAS_PROJECT")) {
        return Ok(DetectedContext::new(
            ProjectContext::new(org, project),
            ContextSource::EnvVars,
        ));
    }

    let cwd = get_working_directory()?;

    // 2. Local storage (.atlas/ in CWD)
    if let Some(ctx) = try_local_storage(&cwd) {
        return Ok(DetectedContext::new(ctx, ContextSource::LocalStorage));
    }

    // 3. Git remote
    if let Some(ctx) = try_git_remote(&cwd) {
        return Ok(DetectedContext::new(ctx, ContextSource::GitRemote));
    }

    // 4. Fallback
    let dir_name = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    Ok(DetectedContext::new(
        ProjectContext::new("global".to_string(), dir_name.to_string()),
        ContextSource::Fallback,
    ))
}

/// Try to detect context from .atlas/ directory in CWD.
///
/// This function checks for the presence of a .atlas/ directory in the provided
/// path and attempts to read org/project configuration from .atlas/config.yaml.
///
/// The config.yaml file is expected to contain simple key-value pairs for:
/// - org: organization name
/// - project: project name
///
/// Both values are required for the context to be considered valid.
///
/// # Arguments
/// * `path` - The directory path to check for .atlas/ configuration
///
/// # Returns
/// * Option<ProjectContext> - Some context if found and valid, None otherwise
fn try_local_storage(path: &Path) -> Option<ProjectContext> {
    let atlas_dir = path.join(".atlas");
    if !atlas_dir.is_dir() {
        return None;
    }

    // Check for config.yaml with org/project
    let config_path = atlas_dir.join("config.yaml");
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            // Simple YAML parsing for org/project
            let mut org: Option<String> = None;
            let mut project: Option<String> = None;

            for line in content.lines() {
                let line = line.trim();
                if let Some(value) = line.strip_prefix("org:") {
                    org = Some(
                        value
                            .trim()
                            .trim_matches('"')
                            .trim_matches('\'')
                            .to_string(),
                    );
                } else if let Some(value) = line.strip_prefix("project:") {
                    project = Some(
                        value
                            .trim()
                            .trim_matches('"')
                            .trim_matches('\'')
                            .to_string(),
                    );
                }
            }

            if let (Some(o), Some(p)) = (org, project) {
                if !o.is_empty() && !p.is_empty() {
                    return Some(ProjectContext::new(o, p));
                }
            }
        }
    }

    None
}

/// Get the working directory, respecting ATLAS_CWD env var.
fn get_working_directory() -> Result<PathBuf, AtlasError> {
    if let Ok(path) = std::env::var("ATLAS_CWD") {
        return Ok(PathBuf::from(path));
    }
    std::env::current_dir().map_err(|e| AtlasError::Context(format!("Failed to get CWD: {}", e)))
}

/// Detect project context from a specific path.
#[allow(dead_code)]
pub fn detect_context_from_path(path: &Path) -> Result<ProjectContext, AtlasError> {
    // Try local storage first
    if let Some(ctx) = try_local_storage(path) {
        return Ok(ctx);
    }

    // Try git remote
    if let Some(ctx) = try_git_remote(path) {
        return Ok(ctx);
    }

    // Fall back to global/<directory_name>
    let dir_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    Ok(ProjectContext::new(
        "global".to_string(),
        dir_name.to_string(),
    ))
}

/// Try to get org/project from git remote URL.
///
/// This function attempts to extract the organization and project names from
/// the git remote URL of the repository located at the provided path.
///
/// It executes 'git remote get-url origin' in the specified directory and
/// parses the resulting URL to extract the org/project components.
///
/// Supported URL formats:
/// - SSH: git@hostname:org/project.git
/// - HTTPS: https://hostname/org/project.git
/// - Both with and without the .git suffix
///
/// # Arguments
/// * `path` - The directory path where the git repository is located
///
/// # Returns
/// * Option<ProjectContext> - Some context if git remote is found and parsable, None otherwise
fn try_git_remote(path: &Path) -> Option<ProjectContext> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_git_url(&url)
}

/// Parse a git URL to extract org and project.
///
/// Supports:
/// - git@github.com:spacelift-io/worker.git
/// - https://github.com/sephriot/atlas.git
/// - git@gitlab.com:org/project.git
/// - https://gitlab.com/org/project
fn parse_git_url(url: &str) -> Option<ProjectContext> {
    // SSH format: git@host:org/project.git
    let ssh_re = Regex::new(r"^git@[^:]+:([^/]+)/([^/]+?)(?:\.git)?$").ok()?;
    if let Some(caps) = ssh_re.captures(url) {
        return Some(ProjectContext::new(
            caps.get(1)?.as_str().to_string(),
            caps.get(2)?.as_str().to_string(),
        ));
    }

    // HTTPS format: https://host/org/project.git
    let https_re = Regex::new(r"^https?://[^/]+/([^/]+)/([^/]+?)(?:\.git)?$").ok()?;
    if let Some(caps) = https_re.captures(url) {
        return Some(ProjectContext::new(
            caps.get(1)?.as_str().to_string(),
            caps.get(2)?.as_str().to_string(),
        ));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ssh_github() {
        let ctx = parse_git_url("git@github.com:spacelift-io/worker.git").unwrap();
        assert_eq!(ctx.org, "spacelift-io");
        assert_eq!(ctx.project, "worker");
    }

    #[test]
    fn test_parse_ssh_gitlab() {
        let ctx = parse_git_url("git@gitlab.com:myorg/myproject.git").unwrap();
        assert_eq!(ctx.org, "myorg");
        assert_eq!(ctx.project, "myproject");
    }

    #[test]
    fn test_parse_ssh_no_git_suffix() {
        let ctx = parse_git_url("git@github.com:org/repo").unwrap();
        assert_eq!(ctx.org, "org");
        assert_eq!(ctx.project, "repo");
    }

    #[test]
    fn test_parse_https_github() {
        let ctx = parse_git_url("https://github.com/sephriot/atlas.git").unwrap();
        assert_eq!(ctx.org, "sephriot");
        assert_eq!(ctx.project, "atlas");
    }

    #[test]
    fn test_parse_https_no_git_suffix() {
        let ctx = parse_git_url("https://github.com/sephriot/atlas").unwrap();
        assert_eq!(ctx.org, "sephriot");
        assert_eq!(ctx.project, "atlas");
    }

    #[test]
    fn test_parse_http_url() {
        let ctx = parse_git_url("http://gitlab.example.com/team/project.git").unwrap();
        assert_eq!(ctx.org, "team");
        assert_eq!(ctx.project, "project");
    }

    #[test]
    fn test_parse_invalid_url() {
        assert!(parse_git_url("not-a-url").is_none());
        assert!(parse_git_url("").is_none());
        assert!(parse_git_url("https://github.com/only-one-part").is_none());
    }

    #[test]
    fn test_project_context_new() {
        let ctx = ProjectContext::new("my-org".to_string(), "my-project".to_string());
        assert_eq!(ctx.org, "my-org");
        assert_eq!(ctx.project, "my-project");
    }

    #[test]
    fn test_detect_context_env_vars_priority() {
        // Save original values
        let orig_org = std::env::var("ATLAS_ORG").ok();
        let orig_project = std::env::var("ATLAS_PROJECT").ok();
        let orig_cwd = std::env::var("ATLAS_CWD").ok();

        // Set test values and point to a non-git directory
        std::env::set_var("ATLAS_ORG", "test-org");
        std::env::set_var("ATLAS_PROJECT", "test-project");
        std::env::set_var("ATLAS_CWD", "/tmp");

        let detected = detect_context_full().unwrap();
        assert_eq!(detected.context.org, "test-org");
        assert_eq!(detected.context.project, "test-project");
        assert_eq!(detected.source, ContextSource::EnvVars);

        // Restore original values
        match orig_org {
            Some(v) => std::env::set_var("ATLAS_ORG", v),
            None => std::env::remove_var("ATLAS_ORG"),
        }
        match orig_project {
            Some(v) => std::env::set_var("ATLAS_PROJECT", v),
            None => std::env::remove_var("ATLAS_PROJECT"),
        }
        match orig_cwd {
            Some(v) => std::env::set_var("ATLAS_CWD", v),
            None => std::env::remove_var("ATLAS_CWD"),
        }
    }

    // ============================================================================
    // ContextSource tests
    // ============================================================================

    #[test]
    fn test_context_source_serialization() {
        assert_eq!(
            serde_json::to_string(&ContextSource::LocalStorage).unwrap(),
            r#""local_storage""#
        );
        assert_eq!(
            serde_json::to_string(&ContextSource::GitRemote).unwrap(),
            r#""git_remote""#
        );
        assert_eq!(
            serde_json::to_string(&ContextSource::EnvVars).unwrap(),
            r#""env_vars""#
        );
        assert_eq!(
            serde_json::to_string(&ContextSource::Fallback).unwrap(),
            r#""fallback""#
        );
    }

    // ============================================================================
    // validate_name tests
    // ============================================================================

    #[test]
    fn test_validate_name_valid() {
        assert!(validate_name("myorg").is_ok());
        assert!(validate_name("my-org").is_ok());
        assert!(validate_name("my_org").is_ok());
        assert!(validate_name("MyOrg123").is_ok());
        assert!(validate_name("a").is_ok());
        assert!(validate_name("1test").is_ok());
    }

    #[test]
    fn test_validate_name_invalid() {
        assert!(validate_name("").is_err());
        assert!(validate_name("-invalid").is_err());
        assert!(validate_name("_invalid").is_err());
        assert!(validate_name("has space").is_err());
        assert!(validate_name("has/slash").is_err());
        assert!(validate_name("has.dot").is_err());
        assert!(validate_name("../traversal").is_err());
    }

    // ============================================================================
    // DetectedContext tests
    // ============================================================================

    #[test]
    fn test_detected_context_is_fallback() {
        let fallback = DetectedContext::new(
            ProjectContext::new("global".into(), "test".into()),
            ContextSource::Fallback,
        );
        assert!(fallback.is_fallback());

        let git = DetectedContext::new(
            ProjectContext::new("org".into(), "proj".into()),
            ContextSource::GitRemote,
        );
        assert!(!git.is_fallback());

        let env = DetectedContext::new(
            ProjectContext::new("org".into(), "proj".into()),
            ContextSource::EnvVars,
        );
        assert!(!env.is_fallback());
    }
}
