//! Comment System — per-section threaded comments for async collaboration.

use super::CollaboratorIdentity;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── Errors ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum CommentError {
    #[error("comment not found: {0}")]
    NotFound(String),
    #[error("cannot reply to a reply (max 1 level of nesting)")]
    TooDeep,
}

// ─── Types ────────────────────────────────────────────────────────────────

/// A comment on a project section or the project as a whole.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub section_id: Option<String>,
    pub element_path: Option<String>,
    pub author: String,
    pub author_name: String,
    pub text: String,
    pub timestamp: String,
    pub resolved: bool,
    pub replies: Vec<Comment>,
}

/// In-memory comment store for a project.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommentStore {
    pub comments: Vec<Comment>,
}

// ─── Operations ───────────────────────────────────────────────────────────

/// Add a top-level comment.
pub fn add_comment(
    store: &mut CommentStore,
    section_id: Option<&str>,
    text: &str,
    author: &CollaboratorIdentity,
) -> Comment {
    let comment = Comment {
        id: uuid::Uuid::new_v4().to_string(),
        section_id: section_id.map(String::from),
        element_path: None,
        author: author.public_key.clone(),
        author_name: author.display_name.clone(),
        text: text.to_string(),
        timestamp: crate::deploy::now_iso8601(),
        resolved: false,
        replies: vec![],
    };
    store.comments.push(comment.clone());
    comment
}

/// Reply to an existing comment.
pub fn reply_to_comment(
    store: &mut CommentStore,
    parent_id: &str,
    text: &str,
    author: &CollaboratorIdentity,
) -> Result<Comment, CommentError> {
    let reply = Comment {
        id: uuid::Uuid::new_v4().to_string(),
        section_id: None,
        element_path: None,
        author: author.public_key.clone(),
        author_name: author.display_name.clone(),
        text: text.to_string(),
        timestamp: crate::deploy::now_iso8601(),
        resolved: false,
        replies: vec![],
    };

    let parent = store
        .comments
        .iter_mut()
        .find(|c| c.id == parent_id)
        .ok_or_else(|| CommentError::NotFound(parent_id.to_string()))?;

    parent.replies.push(reply.clone());
    Ok(reply)
}

/// Mark a comment as resolved.
pub fn resolve_comment(store: &mut CommentStore, comment_id: &str) -> Result<(), CommentError> {
    // Check top-level
    for comment in &mut store.comments {
        if comment.id == comment_id {
            comment.resolved = true;
            return Ok(());
        }
        // Check replies
        for reply in &mut comment.replies {
            if reply.id == comment_id {
                reply.resolved = true;
                return Ok(());
            }
        }
    }
    Err(CommentError::NotFound(comment_id.to_string()))
}

/// Get comments for a specific section (None = project-level).
pub fn get_comments_for_section<'a>(
    store: &'a CommentStore,
    section_id: Option<&'a str>,
) -> Vec<&'a Comment> {
    store
        .comments
        .iter()
        .filter(|c| c.section_id.as_deref() == section_id)
        .collect()
}

/// Save comment store to project directory.
pub fn save_comments(project_dir: &std::path::Path, store: &CommentStore) -> Result<(), String> {
    let path = project_dir.join("collab_comments.json");
    let json =
        serde_json::to_string_pretty(store).map_err(|e| format!("serialize comments: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("write comments: {e}"))
}

/// Load comment store from project directory.
pub fn load_comments(project_dir: &std::path::Path) -> CommentStore {
    let path = project_dir.join("collab_comments.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collab::roles::CollaborationRole;

    fn test_author() -> CollaboratorIdentity {
        CollaboratorIdentity::new(
            "author123".into(),
            "Alice".into(),
            CollaborationRole::Editor,
        )
    }

    fn test_replier() -> CollaboratorIdentity {
        CollaboratorIdentity::new("replier456".into(), "Bob".into(), CollaborationRole::Editor)
    }

    #[test]
    fn test_add_comment_returns_valid() {
        let mut store = CommentStore::default();
        let author = test_author();
        let comment = add_comment(&mut store, Some("hero"), "Nice headline!", &author);

        assert!(!comment.id.is_empty());
        assert!(!comment.timestamp.is_empty());
        assert_eq!(comment.author, "author123");
        assert_eq!(comment.author_name, "Alice");
        assert_eq!(comment.text, "Nice headline!");
        assert!(!comment.resolved);
        assert_eq!(store.comments.len(), 1);
    }

    #[test]
    fn test_reply_to_comment() {
        let mut store = CommentStore::default();
        let author = test_author();
        let comment = add_comment(&mut store, Some("hero"), "Shorten the headline?", &author);

        let replier = test_replier();
        let reply =
            reply_to_comment(&mut store, &comment.id, "Good point, fixing now", &replier).unwrap();

        assert_eq!(reply.author_name, "Bob");
        assert_eq!(store.comments[0].replies.len(), 1);
        assert_eq!(store.comments[0].replies[0].text, "Good point, fixing now");
    }

    #[test]
    fn test_resolve_comment() {
        let mut store = CommentStore::default();
        let author = test_author();
        let comment = add_comment(&mut store, None, "Looks good!", &author);

        resolve_comment(&mut store, &comment.id).unwrap();
        assert!(store.comments[0].resolved);
    }

    #[test]
    fn test_comment_section_association() {
        let mut store = CommentStore::default();
        let author = test_author();

        add_comment(&mut store, Some("hero"), "Hero comment", &author);
        add_comment(&mut store, Some("pricing"), "Pricing comment", &author);
        add_comment(&mut store, None, "Project comment", &author);

        let hero = get_comments_for_section(&store, Some("hero"));
        assert_eq!(hero.len(), 1);
        assert_eq!(hero[0].text, "Hero comment");

        let project = get_comments_for_section(&store, None);
        assert_eq!(project.len(), 1);
        assert_eq!(project[0].text, "Project comment");
    }

    #[test]
    fn test_reply_to_nonexistent_fails() {
        let mut store = CommentStore::default();
        let author = test_author();
        let result = reply_to_comment(&mut store, "nonexistent", "reply", &author);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_nonexistent_fails() {
        let mut store = CommentStore::default();
        let result = resolve_comment(&mut store, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_comment_persistence() {
        let dir = std::env::temp_dir().join(format!("nexus-comments-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut store = CommentStore::default();
        let author = test_author();
        add_comment(&mut store, Some("hero"), "Test comment", &author);

        save_comments(&dir, &store).unwrap();
        let loaded = load_comments(&dir);
        assert_eq!(loaded.comments.len(), 1);
        assert_eq!(loaded.comments[0].text, "Test comment");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
