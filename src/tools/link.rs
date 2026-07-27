use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::context::{detect_context_full, require_explicit_write_context, ProjectContext};
use crate::error::AtlasError;
use crate::locking::ProjectLock;
use crate::models::{Atom, IndexEntry};
use crate::storage::{load_index, read_atom, save_index, write_atom};

use super::reference::{format_atom_reference, parse_atom_reference, AtomRef};

// ============================================================================
// Request/Response types
// ============================================================================

/// Link request parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct LinkRequest {
    /// Source atom: "org/project/K-000001", "project/K-000001", or "K-000001"
    pub source: String,

    /// Target atom: "org/project/K-000001", "project/K-000001", or "K-000001"
    pub target: String,
}

/// Link response.
#[derive(Debug, Clone, Serialize)]
pub struct LinkResponse {
    /// Full source reference: "org/project/K-000001"
    pub source: String,

    /// Full target reference: "org/project/K-000001"
    pub target: String,

    /// False if both atoms already referenced each other
    pub created: bool,
}

/// Unlink response.
#[derive(Debug, Clone, Serialize)]
pub struct UnlinkResponse {
    /// Full source reference: "org/project/K-000001"
    pub source: String,

    /// Full target reference: "org/project/K-000001"
    pub target: String,

    /// False if neither atom referenced the other
    pub removed: bool,
}

// ============================================================================
// Helpers
// ============================================================================

/// Format link relative to source project for storage.
/// Same project: "K-000001"
/// Cross-project: "other-project/K-000001"
fn format_link_for_storage(source_project: &str, target_project: &str, target_id: &str) -> String {
    if source_project == target_project {
        target_id.to_string()
    } else {
        format!("{}/{}", target_project, target_id)
    }
}

/// Lock every project a link touches, ordered so two concurrent links over the
/// same pair cannot each hold the lock the other waits for.
fn lock_projects(refs: [&AtomRef; 2]) -> Result<Vec<ProjectLock>, AtlasError> {
    let mut scopes: Vec<(&str, &str)> = refs
        .iter()
        .map(|r| (r.org.as_str(), r.project.as_str()))
        .collect();
    scopes.sort_unstable();
    scopes.dedup();
    scopes
        .into_iter()
        .map(|(org, project)| ProjectLock::acquire(org, project))
        .collect()
}

/// Stored links predate the current format rules, so compare what a link
/// resolves to rather than how it was written.
fn references(atom: &Atom, owner: &ProjectContext, target: &AtomRef) -> bool {
    atom.links
        .iter()
        .any(|link| parse_atom_reference(link, owner) == *target)
}

fn add_reference(atom: &mut Atom, owner: &ProjectContext, target: &AtomRef) -> bool {
    if references(atom, owner, target) {
        return false;
    }
    atom.links.push(format_link_for_storage(
        &owner.project,
        &target.project,
        &target.id,
    ));
    true
}

fn remove_reference(atom: &mut Atom, owner: &ProjectContext, target: &AtomRef) -> bool {
    let before = atom.links.len();
    atom.links
        .retain(|link| parse_atom_reference(link, owner) != *target);
    atom.links.len() != before
}

fn save_atom(atom_ref: &AtomRef, atom: &mut Atom) -> Result<(), AtlasError> {
    atom.updated_at = Utc::now().date_naive();
    write_atom(&atom_ref.org, &atom_ref.project, atom)?;

    let mut index = load_index(&atom_ref.org, &atom_ref.project)?;
    index.insert_or_replace_entry(IndexEntry::from_atom(atom));
    save_index(&atom_ref.org, &atom_ref.project, &index)
}

fn owner_context(atom_ref: &AtomRef) -> ProjectContext {
    ProjectContext::new(atom_ref.org.clone(), atom_ref.project.clone())
}

fn resolve_pair(req: &LinkRequest) -> Result<(AtomRef, AtomRef), AtlasError> {
    let ctx = require_explicit_write_context(detect_context_full()?)?;
    let source_ref = parse_atom_reference(&req.source, &ctx);
    let target_ref = parse_atom_reference(&req.target, &ctx);

    if source_ref.org != target_ref.org {
        return Err(AtlasError::Validation(
            "Cannot link atoms across different organizations".into(),
        ));
    }

    Ok((source_ref, target_ref))
}

// ============================================================================
// Link tool
// ============================================================================

/// Link two atoms so each one references the other.
pub fn link(req: LinkRequest) -> Result<LinkResponse, AtlasError> {
    let (source_ref, target_ref) = resolve_pair(&req)?;

    if source_ref == target_ref {
        return Err(AtlasError::Validation(
            "Cannot link an atom to itself".into(),
        ));
    }

    let _locks = lock_projects([&source_ref, &target_ref])?;

    let mut source_atom = read_atom(&source_ref.org, &source_ref.project, &source_ref.id)?;
    let mut target_atom = read_atom(&target_ref.org, &target_ref.project, &target_ref.id)?;

    let source_ctx = owner_context(&source_ref);
    let target_ctx = owner_context(&target_ref);

    let forward = add_reference(&mut source_atom, &source_ctx, &target_ref);
    let backward = add_reference(&mut target_atom, &target_ctx, &source_ref);

    if forward {
        save_atom(&source_ref, &mut source_atom)?;
    }
    if backward {
        save_atom(&target_ref, &mut target_atom)?;
    }

    Ok(LinkResponse {
        source: format_atom_reference(&source_ref.org, &source_ref.project, &source_ref.id),
        target: format_atom_reference(&target_ref.org, &target_ref.project, &target_ref.id),
        created: forward || backward,
    })
}

// ============================================================================
// Unlink tool
// ============================================================================

/// Remove the references two atoms hold to each other.
pub fn unlink(req: LinkRequest) -> Result<UnlinkResponse, AtlasError> {
    let (source_ref, target_ref) = resolve_pair(&req)?;

    let _locks = lock_projects([&source_ref, &target_ref])?;

    let mut source_atom = read_atom(&source_ref.org, &source_ref.project, &source_ref.id)?;
    let forward = remove_reference(&mut source_atom, &owner_context(&source_ref), &target_ref);
    if forward {
        save_atom(&source_ref, &mut source_atom)?;
    }

    // A deleted target still leaves its half behind, so its absence is not an error.
    let mut backward = false;
    if let Ok(mut target_atom) = read_atom(&target_ref.org, &target_ref.project, &target_ref.id) {
        backward = remove_reference(&mut target_atom, &owner_context(&target_ref), &source_ref);
        if backward {
            save_atom(&target_ref, &mut target_atom)?;
        }
    }

    Ok(UnlinkResponse {
        source: format_atom_reference(&source_ref.org, &source_ref.project, &source_ref.id),
        target: format_atom_reference(&target_ref.org, &target_ref.project, &target_ref.id),
        removed: forward || backward,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AtomType, Confidence};

    fn atom_ref(project: &str, id: &str) -> AtomRef {
        AtomRef::new("acme".to_string(), project.to_string(), id.to_string())
    }

    fn atom_with_links(links: &[&str]) -> Atom {
        let mut atom = Atom::new(
            "K-000001".to_string(),
            "Title".to_string(),
            AtomType::Note,
            Confidence::High,
            "Summary".to_string(),
        );
        atom.links = links.iter().map(|l| l.to_string()).collect();
        atom
    }

    #[test]
    fn test_format_link_for_storage_same_project() {
        let link = format_link_for_storage("project-a", "project-a", "K-000001");
        assert_eq!(link, "K-000001");
    }

    #[test]
    fn test_format_link_for_storage_cross_project() {
        let link = format_link_for_storage("project-a", "project-b", "K-000001");
        assert_eq!(link, "project-b/K-000001");
    }

    #[test]
    fn test_add_reference_uses_storage_format_of_the_owner() {
        let owner = ProjectContext::new("acme".to_string(), "atlas".to_string());
        let mut atom = atom_with_links(&[]);

        assert!(add_reference(
            &mut atom,
            &owner,
            &atom_ref("atlas", "K-000002")
        ));
        assert!(add_reference(
            &mut atom,
            &owner,
            &atom_ref("other", "K-000003")
        ));
        assert_eq!(atom.links, vec!["K-000002", "other/K-000003"]);
    }

    #[test]
    fn test_add_reference_recognizes_an_equivalent_stored_form() {
        let owner = ProjectContext::new("acme".to_string(), "atlas".to_string());
        let mut atom = atom_with_links(&["acme/atlas/K-000002"]);

        assert!(!add_reference(
            &mut atom,
            &owner,
            &atom_ref("atlas", "K-000002")
        ));
        assert_eq!(atom.links, vec!["acme/atlas/K-000002"]);
    }

    #[test]
    fn test_remove_reference_matches_any_stored_form() {
        let owner = ProjectContext::new("acme".to_string(), "atlas".to_string());
        let mut atom = atom_with_links(&["acme/atlas/K-000002", "K-000003"]);

        assert!(remove_reference(
            &mut atom,
            &owner,
            &atom_ref("atlas", "K-000002")
        ));
        assert_eq!(atom.links, vec!["K-000003"]);
        assert!(!remove_reference(
            &mut atom,
            &owner,
            &atom_ref("atlas", "K-000009")
        ));
    }
}
