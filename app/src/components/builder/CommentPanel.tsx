/**
 * CommentPanel — side panel for async collaboration comments.
 *
 * Shows threaded comments per section with resolve and reply actions.
 * All inline styles per project convention.
 */

import { useState, useCallback, useEffect } from "react";
import {
  builderCollabAddComment,
  builderCollabGetComments,
  builderCollabResolveComment,
  type CollabComment,
} from "../../api/backend";

const C = {
  bg: "#0a0e14",
  surface: "#111820",
  surfaceAlt: "#0d1219",
  border: "#1a2332",
  text: "#e2e8f0",
  muted: "#94a3b8",
  dim: "#3e4c5e",
  accent: "#00d4aa",
  accentDim: "rgba(0,212,170,0.10)",
  green: "#22c55e",
  sans: "system-ui,-apple-system,sans-serif",
};

interface CommentPanelProps {
  projectId: string;
  sectionId?: string;
}

export default function CommentPanel({ projectId, sectionId }: CommentPanelProps) {
  const [comments, setComments] = useState<CollabComment[]>([]);
  const [newText, setNewText] = useState("");
  const [loading, setLoading] = useState(false);

  const loadComments = useCallback(async () => {
    try {
      const data = await builderCollabGetComments(projectId, sectionId || null);
      setComments(data);
    } catch (e: any) {
      console.error("Load comments:", e);
    }
  }, [projectId, sectionId]);

  useEffect(() => {
    loadComments();
  }, [loadComments]);

  const addComment = useCallback(async () => {
    if (!newText.trim()) return;
    setLoading(true);
    try {
      await builderCollabAddComment(projectId, sectionId || null, newText.trim());
      setNewText("");
      await loadComments();
    } catch (e: any) {
      console.error("Add comment:", e);
    }
    setLoading(false);
  }, [projectId, sectionId, newText, loadComments]);

  const resolveComment = useCallback(async (commentId: string) => {
    try {
      await builderCollabResolveComment(projectId, commentId);
      await loadComments();
    } catch (e: any) {
      console.error("Resolve comment:", e);
    }
  }, [projectId, loadComments]);

  const unresolvedCount = comments.filter((c) => !c.resolved).length;

  return (
    <div style={{
      background: C.surfaceAlt, border: `1px solid ${C.border}`, borderRadius: 6,
      padding: 12, fontFamily: C.sans, minWidth: 260, maxHeight: 400, overflowY: "auto",
    }}>
      {/* Header */}
      <div style={{
        display: "flex", justifyContent: "space-between", alignItems: "center",
        marginBottom: 10,
      }}>
        <span style={{ color: C.text, fontSize: 11, fontWeight: 600 }}>
          Comments{unresolvedCount > 0 ? ` (${unresolvedCount})` : ""}
        </span>
        {sectionId && (
          <span style={{ color: C.dim, fontSize: 9 }}>{sectionId}</span>
        )}
      </div>

      {/* Comment list */}
      {comments.length === 0 && (
        <div style={{ color: C.dim, fontSize: 10, padding: "8px 0" }}>
          No comments yet
        </div>
      )}

      {comments.map((comment) => (
        <div key={comment.id} style={{
          marginBottom: 8, paddingBottom: 8,
          borderBottom: `1px solid ${C.border}`,
          opacity: comment.resolved ? 0.5 : 1,
        }}>
          {/* Section badge */}
          {comment.section_id && !sectionId && (
            <div style={{ fontSize: 8, color: C.accent, marginBottom: 2 }}>
              {comment.section_id}
            </div>
          )}

          {/* Author + text */}
          <div style={{ fontSize: 10 }}>
            <span style={{ color: C.text, fontWeight: 600 }}>{comment.author_name}: </span>
            <span style={{ color: C.muted }}>
              {comment.resolved ? <s>{comment.text}</s> : comment.text}
            </span>
          </div>

          {/* Replies */}
          {comment.replies && comment.replies.length > 0 && (
            <div style={{ marginTop: 4, paddingLeft: 12, borderLeft: `2px solid ${C.border}` }}>
              {comment.replies.map((reply) => (
                <div key={reply.id} style={{ fontSize: 9, marginBottom: 2 }}>
                  <span style={{ color: C.text, fontWeight: 600 }}>{reply.author_name}: </span>
                  <span style={{ color: C.muted }}>{reply.text}</span>
                </div>
              ))}
            </div>
          )}

          {/* Actions */}
          {!comment.resolved && (
            <div style={{ marginTop: 3 }}>
              <button
                onClick={() => resolveComment(comment.id)}
                style={{
                  background: "transparent", border: "none", color: C.green,
                  fontSize: 8, cursor: "pointer", padding: 0,
                }}
              >
                Resolve
              </button>
            </div>
          )}
        </div>
      ))}

      {/* Add comment input */}
      <div style={{ display: "flex", gap: 4, marginTop: 8 }}>
        <input
          value={newText}
          onChange={(e) => setNewText(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && addComment()}
          placeholder="Add comment..."
          style={{
            flex: 1, background: C.surface, border: `1px solid ${C.border}`,
            borderRadius: 3, padding: "4px 8px", color: C.text, fontSize: 10,
            outline: "none", fontFamily: C.sans,
          }}
        />
        <button
          onClick={addComment}
          disabled={loading || !newText.trim()}
          style={{
            background: C.accentDim, border: `1px solid rgba(0,212,170,0.2)`,
            borderRadius: 3, padding: "4px 8px", color: C.accent, fontSize: 9,
            cursor: loading ? "default" : "pointer",
            opacity: loading || !newText.trim() ? 0.5 : 1,
          }}
        >
          Send
        </button>
      </div>
    </div>
  );
}
