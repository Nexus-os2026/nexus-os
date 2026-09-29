/**
 * collab-sync.ts — real-time collaboration for Nexus Builder (unavailable).
 *
 * P0 item D: unavailable in Phase Zero. Collaboration would open an
 * unauthenticated, plaintext WebSocket (y-websocket) to a caller-supplied
 * server directly from the privileged app origin. There is no governed
 * transport for it and the restrictive CSP's `connect-src` does not permit it.
 * No production module imports this file, and it imports neither `yjs` nor
 * `y-websocket`; neither package is a dependency of the app.
 */

// ─── Types ────────────────────────────────────────────────────────────────

export interface CollaboratorIdentity {
  public_key: string;
  display_name: string;
  color: string;
  role: "Owner" | "Editor" | "Commenter" | "Viewer";
}

export interface PresenceState {
  user: CollaboratorIdentity;
  selectedSection: string | null;
  activePanel: string | null;
}

/** No collaboration session can exist in Phase Zero. */
export type CollabSyncHandle = never;

// ─── Init ─────────────────────────────────────────────────────────────────

/**
 * Fails closed: never connects and never constructs a WebSocket provider.
 */
export function initCollabSync(
  _serverUrl: string,
  _roomName: string,
  _identity: CollaboratorIdentity
): CollabSyncHandle {
  throw new Error(
    "Nexus Builder real-time collaboration is unavailable in Phase Zero"
  );
}
