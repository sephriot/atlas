use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config::get_project_path;
use crate::context::detect_context_full;
use crate::error::AtlasError;
use crate::locking::ProjectLock;
use crate::models::{Atom, AtomType, Confidence, IndexEntry};
use crate::storage::{ensure_project_exists, load_index, read_atom, save_index, write_atom};

use super::reference::{format_atom_reference, parse_atom_reference};

/// Check if a string looks like a stringified JSON array and return parsed version if so.
fn detect_stringified_array(value: &str) -> Option<Vec<String>> {
    let trimmed = value.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        serde_json::from_str::<Vec<String>>(trimmed).ok()
    } else {
        None
    }
}

/// Validate that array fields don't contain stringified JSON arrays.
/// Returns an error with the correct format if stringified arrays are detected.
fn validate_array_fields(req: &AtomWriteRequest) -> Result<(), AtlasError> {
    let mut errors = Vec::new();

    // Check each array field for stringified arrays
    if let Some(ref tags) = req.tags {
        for (i, tag) in tags.iter().enumerate() {
            if let Some(parsed) = detect_stringified_array(tag) {
                errors.push(format!(
                    "tags[{}] appears to be a stringified array. You passed: tags: {:?}. Correct format: tags: {:?}",
                    i, tags, parsed
                ));
                break; // One error per field is enough
            }
        }
    }

    if let Some(ref sources) = req.sources {
        for (i, source) in sources.iter().enumerate() {
            if let Some(parsed) = detect_stringified_array(source) {
                errors.push(format!(
                    "sources[{}] appears to be a stringified array. You passed: sources: {:?}. Correct format: sources: {:?}",
                    i, sources, parsed
                ));
                break;
            }
        }
    }

    if let Some(ref links) = req.links {
        for (i, link) in links.iter().enumerate() {
            if let Some(parsed) = detect_stringified_array(link) {
                errors.push(format!(
                    "links[{}] appears to be a stringified array. You passed: links: {:?}. Correct format: links: {:?}",
                    i, links, parsed
                ));
                break;
            }
        }
    }

    if let Some(ref pitfalls) = req.pitfalls {
        for (i, pitfall) in pitfalls.iter().enumerate() {
            if let Some(parsed) = detect_stringified_array(pitfall) {
                errors.push(format!(
                    "pitfalls[{}] appears to be a stringified array. You passed: pitfalls: {:?}. Correct format: pitfalls: {:?}",
                    i, pitfalls, parsed
                ));
                break;
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(AtlasError::Validation(format!(
            "Array fields must be JSON arrays, not stringified arrays:\n{}",
            errors.join("\n")
        )))
    }
}

/// Atom write request parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct AtomWriteRequest {
    /// Short descriptive title
    pub title: String,

    /// Type of knowledge. In JSON this must be a quoted string: "note", "gotcha", "recipe", or "decision" (not a bare identifier).
    #[serde(rename = "type")]
    pub atom_type: AtomType,

    /// Confidence level. In JSON this must be a quoted string: "high", "medium", or "low".
    pub confidence: Confidence,

    /// Brief explanation
    pub summary: String,

    /// Extended content (optional)
    #[serde(default)]
    pub details: Option<String>,

    /// Potential pitfalls (optional)
    #[serde(default)]
    pub pitfalls: Option<Vec<String>>,

    /// Keywords for search (optional)
    #[serde(default)]
    pub tags: Option<Vec<String>>,

    /// References - simple strings (paths or URLs)
    #[serde(default)]
    pub sources: Option<Vec<String>>,

    /// Related atoms - supports bare id, project/id, or org/project/id format
    #[serde(default)]
    pub links: Option<Vec<String>>,
}

/// Atom write response.
#[derive(Debug, Clone, Serialize)]
pub struct AtomWriteResult {
    /// Full atom reference: "org/project/K-000001"
    pub id: String,
    pub created: bool,
}

/// Create a new atom.
pub fn create_atom(req: AtomWriteRequest) -> Result<AtomWriteResult, AtlasError> {
    write_atom_request(None, req)
}

/// Update an existing atom by ID.
pub fn update_atom(id: String, req: AtomWriteRequest) -> Result<AtomWriteResult, AtlasError> {
    write_atom_request(Some(id), req)
}

fn write_atom_request(
    id: Option<String>,
    req: AtomWriteRequest,
) -> Result<AtomWriteResult, AtlasError> {
    // Validate array fields aren't stringified JSON
    validate_array_fields(&req)?;

    let detected = detect_context_full()?;
    let ctx = detected.context;

    // Determine org/project based on whether this is an update or create
    let (target_org, target_project) = if let Some(ref id) = id {
        // Update: parse full path from id
        let atom_ref = parse_atom_reference(id, &ctx);
        (atom_ref.org, atom_ref.project)
    } else {
        // Create: use detected context
        (ctx.org.clone(), ctx.project.clone())
    };

    // Validate links before acquiring lock
    if let Some(ref links) = req.links {
        validate_links(&target_org, links, &ctx)?;
    }

    let _lock = ProjectLock::acquire(&target_org, &target_project)?;

    // Ensure project exists
    ensure_project_exists(&target_org, &target_project)?;

    let mut index = load_index(&target_org, &target_project)?;

    let (atom, created) = if let Some(ref id_str) = id {
        // Update existing - re-parse to get just the ID part
        let atom_ref = parse_atom_reference(id_str, &ctx);
        let mut atom = read_atom(&target_org, &target_project, &atom_ref.id)?;
        atom.title = req.title;
        atom.atom_type = req.atom_type;
        atom.confidence = req.confidence;
        atom.summary = req.summary;
        atom.details = req.details;
        atom.pitfalls = req.pitfalls.unwrap_or_default();
        atom.tags = req.tags.unwrap_or_default();
        atom.sources = req.sources.unwrap_or_default();
        atom.links = req.links.unwrap_or_default();
        atom.updated_at = Utc::now().date_naive();
        (atom, false)
    } else {
        // Create new
        let id = index.generate_id();
        let mut atom = Atom::new(id, req.title, req.atom_type, req.confidence, req.summary);
        atom.details = req.details;
        atom.pitfalls = req.pitfalls.unwrap_or_default();
        atom.tags = req.tags.unwrap_or_default();
        atom.sources = req.sources.unwrap_or_default();
        atom.links = req.links.unwrap_or_default();
        (atom, true)
    };

    // Write atom
    write_atom(&target_org, &target_project, &atom)?;

    // Update index
    index.insert_or_replace_entry(IndexEntry::from_atom(&atom));
    save_index(&target_org, &target_project, &index)?;

    Ok(AtomWriteResult {
        id: format_atom_reference(&target_org, &target_project, &atom.id),
        created,
    })
}

/// Validate links - reject cross-org links, allow cross-project within same org.
///
/// Accepts:
/// - "K-000001" (bare id, same project)
/// - "project/K-000001" (cross-project within same org)
/// - "org/project/K-000001" (full path, org must match target_org)
fn validate_links(
    target_org: &str,
    links: &[String],
    ctx: &crate::context::ProjectContext,
) -> Result<(), AtlasError> {
    for link in links {
        let atom_ref = parse_atom_reference(link, ctx);

        // Reject cross-org links
        if atom_ref.org != target_org {
            return Err(AtlasError::Validation(format!(
                "Cross-org links not allowed: '{}' references org '{}', expected '{}'",
                link, atom_ref.org, target_org
            )));
        }

        // Verify target project exists in the org
        let project_path = get_project_path(&atom_ref.org, &atom_ref.project)?;
        if !project_path.exists() {
            return Err(AtlasError::Validation(format!(
                "Link target project '{}' does not exist in org '{}'",
                atom_ref.project, atom_ref.org
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(
        tags: Option<Vec<String>>,
        sources: Option<Vec<String>>,
        links: Option<Vec<String>>,
        pitfalls: Option<Vec<String>>,
    ) -> AtomWriteRequest {
        AtomWriteRequest {
            title: "Test".into(),
            atom_type: AtomType::Note,
            confidence: Confidence::Medium,
            summary: "Test summary".into(),
            details: None,
            pitfalls,
            tags,
            sources,
            links,
        }
    }

    #[test]
    fn test_atom_write_request_deserializes_minimal_json() {
        let json = r#"{"title":"T","type":"recipe","confidence":"high","summary":"S"}"#;
        let req: AtomWriteRequest = serde_json::from_str(json).expect("valid JSON");
        assert_eq!(req.title, "T");
        assert_eq!(req.atom_type, AtomType::Recipe);
        assert_eq!(req.confidence, Confidence::High);
        assert_eq!(req.summary, "S");
    }

    #[test]
    fn test_atom_write_request_invalid_json_unquoted_type_is_error() {
        let json = r#"{"title":"T","type":recipe,"confidence":"high","summary":"S"}"#;
        assert!(serde_json::from_str::<AtomWriteRequest>(json).is_err());
    }

    #[test]
    fn test_detect_stringified_array_valid() {
        let result = detect_stringified_array(r#"["api", "rust"]"#);
        assert_eq!(result, Some(vec!["api".to_string(), "rust".to_string()]));
    }

    #[test]
    fn test_detect_stringified_array_with_whitespace() {
        let result = detect_stringified_array(r#"  ["api", "rust"]  "#);
        assert_eq!(result, Some(vec!["api".to_string(), "rust".to_string()]));
    }

    #[test]
    fn test_detect_stringified_array_not_array() {
        assert_eq!(detect_stringified_array("just a string"), None);
        assert_eq!(detect_stringified_array("api"), None);
        assert_eq!(detect_stringified_array(""), None);
    }

    #[test]
    fn test_detect_stringified_array_invalid_json() {
        // Starts with [ but not valid JSON
        assert_eq!(detect_stringified_array("[not valid json"), None);
    }

    #[test]
    fn test_validate_array_fields_valid() {
        let req = make_request(
            Some(vec!["api".into(), "rust".into()]),
            Some(vec!["src/lib.rs".into()]),
            None,
            None,
        );
        assert!(validate_array_fields(&req).is_ok());
    }

    #[test]
    fn test_validate_array_fields_stringified_tags() {
        let req = make_request(Some(vec![r#"["api", "rust"]"#.into()]), None, None, None);
        let err = validate_array_fields(&req).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("tags[0]"));
        assert!(msg.contains("stringified array"));
        assert!(msg.contains(r#"Correct format: tags: ["api", "rust"]"#));
    }

    #[test]
    fn test_validate_array_fields_stringified_sources() {
        let req = make_request(
            None,
            Some(vec![r#"["src/lib.rs", "src/main.rs"]"#.into()]),
            None,
            None,
        );
        let err = validate_array_fields(&req).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("sources[0]"));
        assert!(msg.contains("stringified array"));
    }

    #[test]
    fn test_validate_array_fields_none_values() {
        let req = make_request(None, None, None, None);
        assert!(validate_array_fields(&req).is_ok());
    }
}
