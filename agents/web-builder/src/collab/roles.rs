//! Role-Based Access Control — defines collaboration roles and permissions.

use serde::{Deserialize, Serialize};

// ─── Roles ────────────────────────────────────────────────────────────────

/// Collaboration role for a participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollaborationRole {
    Owner,
    Editor,
    Commenter,
    Viewer,
}

impl std::fmt::Display for CollaborationRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Owner => write!(f, "Owner"),
            Self::Editor => write!(f, "Editor"),
            Self::Commenter => write!(f, "Commenter"),
            Self::Viewer => write!(f, "Viewer"),
        }
    }
}

// ─── Actions ──────────────────────────────────────────────────────────────

/// Actions that can be performed in a collaboration session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollabAction {
    EditContent,
    EditTokens,
    EditTheme,
    GenerateVariants,
    Deploy,
    ManageRoles,
    AddComment,
    ResolveComment,
    ViewPreview,
    ExportProject,
    RunQualityCheck,
    AddBackend,
}

// ─── Permission Check ─────────────────────────────────────────────────────

/// Check if a role has permission to perform an action.
pub fn check_permission(role: &CollaborationRole, action: &CollabAction) -> bool {
    match (role, action) {
        // Owner can do everything
        (CollaborationRole::Owner, _) => true,

        // Editor: edit, comment, resolve, view, export, quality
        (CollaborationRole::Editor, CollabAction::EditContent) => true,
        (CollaborationRole::Editor, CollabAction::EditTokens) => true,
        (CollaborationRole::Editor, CollabAction::EditTheme) => true,
        (CollaborationRole::Editor, CollabAction::GenerateVariants) => true,
        (CollaborationRole::Editor, CollabAction::AddComment) => true,
        (CollaborationRole::Editor, CollabAction::ResolveComment) => true,
        (CollaborationRole::Editor, CollabAction::ViewPreview) => true,
        (CollaborationRole::Editor, CollabAction::ExportProject) => true,
        (CollaborationRole::Editor, CollabAction::RunQualityCheck) => true,
        // Editor cannot: Deploy, ManageRoles, AddBackend
        (CollaborationRole::Editor, _) => false,

        // Commenter: comment + view
        (CollaborationRole::Commenter, CollabAction::AddComment) => true,
        (CollaborationRole::Commenter, CollabAction::ViewPreview) => true,
        (CollaborationRole::Commenter, _) => false,

        // Viewer: view only
        (CollaborationRole::Viewer, CollabAction::ViewPreview) => true,
        (CollaborationRole::Viewer, _) => false,
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_owner_can_do_everything() {
        let all_actions = [
            CollabAction::EditContent,
            CollabAction::EditTokens,
            CollabAction::EditTheme,
            CollabAction::GenerateVariants,
            CollabAction::Deploy,
            CollabAction::ManageRoles,
            CollabAction::AddComment,
            CollabAction::ResolveComment,
            CollabAction::ViewPreview,
            CollabAction::ExportProject,
            CollabAction::RunQualityCheck,
            CollabAction::AddBackend,
        ];
        for action in &all_actions {
            assert!(
                check_permission(&CollaborationRole::Owner, action),
                "Owner should have permission for {action:?}"
            );
        }
    }

    #[test]
    fn test_editor_cannot_deploy() {
        assert!(!check_permission(
            &CollaborationRole::Editor,
            &CollabAction::Deploy,
        ));
    }

    #[test]
    fn test_editor_cannot_manage_roles() {
        assert!(!check_permission(
            &CollaborationRole::Editor,
            &CollabAction::ManageRoles,
        ));
    }

    #[test]
    fn test_editor_cannot_add_backend() {
        assert!(!check_permission(
            &CollaborationRole::Editor,
            &CollabAction::AddBackend,
        ));
    }

    #[test]
    fn test_commenter_can_only_comment_and_view() {
        assert!(check_permission(
            &CollaborationRole::Commenter,
            &CollabAction::AddComment,
        ));
        assert!(check_permission(
            &CollaborationRole::Commenter,
            &CollabAction::ViewPreview,
        ));
        assert!(!check_permission(
            &CollaborationRole::Commenter,
            &CollabAction::EditContent,
        ));
        assert!(!check_permission(
            &CollaborationRole::Commenter,
            &CollabAction::EditTokens,
        ));
        assert!(!check_permission(
            &CollaborationRole::Commenter,
            &CollabAction::Deploy,
        ));
        assert!(!check_permission(
            &CollaborationRole::Commenter,
            &CollabAction::ResolveComment,
        ));
    }

    #[test]
    fn test_viewer_can_only_view() {
        assert!(check_permission(
            &CollaborationRole::Viewer,
            &CollabAction::ViewPreview,
        ));
        assert!(!check_permission(
            &CollaborationRole::Viewer,
            &CollabAction::AddComment,
        ));
        assert!(!check_permission(
            &CollaborationRole::Viewer,
            &CollabAction::EditContent,
        ));
        assert!(!check_permission(
            &CollaborationRole::Viewer,
            &CollabAction::Deploy,
        ));
    }

    #[test]
    fn test_permission_matrix_complete() {
        let roles = [
            CollaborationRole::Owner,
            CollaborationRole::Editor,
            CollaborationRole::Commenter,
            CollaborationRole::Viewer,
        ];
        let actions = [
            CollabAction::EditContent,
            CollabAction::EditTokens,
            CollabAction::EditTheme,
            CollabAction::GenerateVariants,
            CollabAction::Deploy,
            CollabAction::ManageRoles,
            CollabAction::AddComment,
            CollabAction::ResolveComment,
            CollabAction::ViewPreview,
            CollabAction::ExportProject,
            CollabAction::RunQualityCheck,
            CollabAction::AddBackend,
        ];

        // Every (role, action) pair must have a defined result (no panics)
        for role in &roles {
            for action in &actions {
                let _ = check_permission(role, action);
            }
        }

        // Verify specific expected counts per role
        let count = |role: &CollaborationRole| -> usize {
            actions.iter().filter(|a| check_permission(role, a)).count()
        };
        assert_eq!(count(&CollaborationRole::Owner), 12); // all 12
        assert_eq!(count(&CollaborationRole::Editor), 9); // 12 - deploy - manage_roles - add_backend
        assert_eq!(count(&CollaborationRole::Commenter), 2); // comment + view
        assert_eq!(count(&CollaborationRole::Viewer), 1); // view only
    }
}
