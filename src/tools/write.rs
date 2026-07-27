use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config::get_project_path;
use crate::context::{detect_context_full, require_explicit_write_context, ProjectContext};
use crate::error::AtlasError;
use crate::locking::ProjectLock;
use crate::models::{Atom, AtomType, Confidence, IndexEntry};
use crate::storage::{ensure_project_exists, load_index, read_atom, save_index, write_atom};

use super::link::{link, LinkRequest};
use super::reference::{format_atom_reference, parse_atom_reference, AtomRef};

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

#[derive(Debug, Clone)]
pub struct AtomUpdateRequest {
    pub title: Option<String>,
    pub atom_type: Option<AtomType>,
    pub confidence: Option<Confidence>,
    pub summary: Option<String>,
    pub details: Option<String>,
    pub clear_details: bool,
    pub pitfalls: Option<Vec<String>>,
    pub clear_pitfalls: bool,
    pub tags: Option<Vec<String>>,
    pub clear_tags: bool,
    pub sources: Option<Vec<String>>,
    pub clear_sources: bool,
}

/// Atom write response.
#[derive(Debug, Clone, Serialize)]
pub struct AtomWriteResult {
    /// Full atom reference: "org/project/K-000001"
    pub id: String,
    pub created: bool,

    /// Atoms this one references after the write. Reported on update as well as
    /// create, because rewriting what an atom says can outdate why it was linked.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<String>,
}

/// Create a new atom.
pub fn create_atom(req: AtomWriteRequest) -> Result<AtomWriteResult, AtlasError> {
    write_atom_request(req)
}

/// Update an existing atom by ID.
pub fn update_atom(id: String, req: AtomUpdateRequest) -> Result<AtomWriteResult, AtlasError> {
    let detected = detect_context_full()?;
    let ctx = require_explicit_write_context(detected)?;
    let atom_ref = parse_atom_reference(&id, &ctx);

    let _lock = ProjectLock::acquire(&atom_ref.org, &atom_ref.project)?;
    let mut atom = read_atom(&atom_ref.org, &atom_ref.project, &atom_ref.id)?;
    let mut index = load_index(&atom_ref.org, &atom_ref.project)?;

    if let Some(title) = req.title {
        atom.title = title;
    }
    if let Some(atom_type) = req.atom_type {
        atom.atom_type = atom_type;
    }
    if let Some(confidence) = req.confidence {
        atom.confidence = confidence;
    }
    if let Some(summary) = req.summary {
        atom.summary = summary;
    }
    if let Some(details) = req.details {
        atom.details = Some(details);
    } else if req.clear_details {
        atom.details = None;
    }
    apply_vec_update(&mut atom.pitfalls, req.pitfalls, req.clear_pitfalls);
    apply_vec_update(&mut atom.tags, req.tags, req.clear_tags);
    apply_vec_update(&mut atom.sources, req.sources, req.clear_sources);
    atom.updated_at = Utc::now().date_naive();

    write_atom(&atom_ref.org, &atom_ref.project, &atom)?;
    index.insert_or_replace_entry(IndexEntry::from_atom(&atom));
    save_index(&atom_ref.org, &atom_ref.project, &index)?;

    Ok(AtomWriteResult {
        id: format_atom_reference(&atom_ref.org, &atom_ref.project, &atom.id),
        created: false,
        links: edge_references(&atom_ref, &atom.links),
    })
}

/// Edges as unambiguous full references, so a caller reviewing them need not resolve a
/// bare id against the atom's own project.
fn edge_references(atom_ref: &AtomRef, links: &[String]) -> Vec<String> {
    let owner = ProjectContext::new(atom_ref.org.clone(), atom_ref.project.clone());
    links
        .iter()
        .map(|link| parse_atom_reference(link, &owner).to_full_path())
        .collect()
}

fn apply_vec_update(target: &mut Vec<String>, replacement: Option<Vec<String>>, clear: bool) {
    if let Some(replacement) = replacement {
        *target = replacement;
    } else if clear {
        target.clear();
    }
}

fn write_atom_request(req: AtomWriteRequest) -> Result<AtomWriteResult, AtlasError> {
    // Validate array fields aren't stringified JSON
    validate_array_fields(&req)?;

    let detected = detect_context_full()?;
    let ctx = require_explicit_write_context(detected)?;

    let target_org = ctx.org.clone();
    let target_project = ctx.project.clone();

    // Resolve every peer before the atom exists, so a bad reference fails the
    // whole create rather than leaving a new atom holding some of its edges.
    let peers = resolve_links(&target_org, req.links.as_deref().unwrap_or_default(), &ctx)?;

    let atom_id = {
        let _lock = ProjectLock::acquire(&target_org, &target_project)?;

        // Ensure project exists
        ensure_project_exists(&target_org, &target_project)?;

        let mut index = load_index(&target_org, &target_project)?;

        let id = index.generate_id();
        let mut atom = Atom::new(id, req.title, req.atom_type, req.confidence, req.summary);
        atom.details = req.details;
        atom.pitfalls = req.pitfalls.unwrap_or_default();
        atom.tags = req.tags.unwrap_or_default();
        atom.sources = req.sources.unwrap_or_default();

        // Write atom
        write_atom(&target_org, &target_project, &atom)?;

        // Update index
        index.insert_or_replace_entry(IndexEntry::from_atom(&atom));
        save_index(&target_org, &target_project, &index)?;

        atom.id
    };

    // Edges go through link so each one lands on both atoms; the project lock above
    // is released by now because linking takes the locks it needs itself.
    let id = format_atom_reference(&target_org, &target_project, &atom_id);
    let mut links = Vec::new();
    for peer in peers {
        let peer = peer.to_full_path();
        link(LinkRequest {
            source: id.clone(),
            target: peer.clone(),
        })?;
        links.push(peer);
    }

    Ok(AtomWriteResult {
        id,
        created: true,
        links,
    })
}

/// Resolve requested links to atoms that exist in the same org.
///
/// Accepts:
/// - "K-000001" (bare id, same project)
/// - "project/K-000001" (cross-project within same org)
/// - "org/project/K-000001" (full path, org must match target_org)
fn resolve_links(
    target_org: &str,
    links: &[String],
    ctx: &ProjectContext,
) -> Result<Vec<AtomRef>, AtlasError> {
    let mut resolved: Vec<AtomRef> = Vec::new();

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

        read_atom(&atom_ref.org, &atom_ref.project, &atom_ref.id)?;

        if !resolved.contains(&atom_ref) {
            resolved.push(atom_ref);
        }
    }

    Ok(resolved)
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
