/**
 * collab-sync.ts — Yjs CRDT sync for Nexus Builder collaboration.
 *
 * Manages the Yjs document, WebSocket provider, and awareness (presence).
 * The Yjs document contains shared state: content slots, token overrides,
 * and comments. HTML is always re-assembled from resolved CRDT state.
 */

import * as Y from "yjs";
import { WebsocketProvider } from "y-websocket";

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

export interface CollabSyncHandle {
  ydoc: Y.Doc;
  provider: WebsocketProvider;
  sections: Y.Map<Y.Map<string>>;
  tokens: Y.Map<string>;
  comments: Y.Array<any>;
  destroy: () => void;
}

// ─── Init ─────────────────────────────────────────────────────────────────

/**
 * Initialize Yjs collaboration sync.
 *
 * P0 item D: unavailable in Phase Zero. This would open an unauthenticated,
 * plaintext WebSocket to a caller-supplied server directly from the privileged
 * app origin. There is no governed transport for it, the restrictive CSP's
 * `connect-src` does not permit it, and no code path reaches it today. It fails
 * closed rather than connecting; a `WebsocketProvider` is never constructed.
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

// ─── Presence Helpers ─────────────────────────────────────────────────────

/**
 * Update the local user's selected section in awareness.
 */
export function updatePresenceSection(
  handle: CollabSyncHandle,
  sectionId: string | null
): void {
  const current = handle.provider.awareness.getLocalState() as PresenceState | null;
  if (current) {
    handle.provider.awareness.setLocalState({
      ...current,
      selectedSection: sectionId,
    });
  }
}

/**
 * Get all remote users' presence states.
 */
export function getRemotePresence(handle: CollabSyncHandle): PresenceState[] {
  const states: PresenceState[] = [];
  handle.provider.awareness.getStates().forEach((state, clientId) => {
    if (clientId !== handle.ydoc.clientID && state?.user) {
      states.push(state as PresenceState);
    }
  });
  return states;
}

/**
 * Subscribe to presence changes (other users joining/leaving/moving).
 */
export function onPresenceChange(
  handle: CollabSyncHandle,
  callback: (states: PresenceState[]) => void
): () => void {
  const handler = () => callback(getRemotePresence(handle));
  handle.provider.awareness.on("change", handler);
  return () => handle.provider.awareness.off("change", handler);
}

// ─── Content Sync ─────────────────────────────────────────────────────────

/**
 * Update a slot value in the shared document.
 */
export function syncSlotUpdate(
  handle: CollabSyncHandle,
  sectionId: string,
  slotName: string,
  value: string
): void {
  let section = handle.sections.get(sectionId);
  if (!section) {
    section = new Y.Map<string>();
    handle.sections.set(sectionId, section);
  }
  section.set(slotName, value);
}

/**
 * Update a token value in the shared document.
 */
export function syncTokenUpdate(
  handle: CollabSyncHandle,
  tokenName: string,
  value: string
): void {
  handle.tokens.set(tokenName, value);
}

/**
 * Subscribe to content changes from remote users.
 */
export function onContentChange(
  handle: CollabSyncHandle,
  callback: (sectionId: string, slotName: string, value: string) => void
): () => void {
  const handler = (event: Y.YMapEvent<Y.Map<string>>) => {
    event.changes.keys.forEach((_change, sectionId) => {
      const section = handle.sections.get(sectionId);
      if (section) {
        section.forEach((value, slotName) => {
          callback(sectionId, slotName, value);
        });
      }
    });
  };
  handle.sections.observe(handler);
  return () => handle.sections.unobserve(handler);
}

/**
 * Subscribe to token changes from remote users.
 */
export function onTokenChange(
  handle: CollabSyncHandle,
  callback: (tokenName: string, value: string) => void
): () => void {
  const handler = (event: Y.YMapEvent<string>) => {
    event.changes.keys.forEach((_change, tokenName) => {
      const value = handle.tokens.get(tokenName);
      if (value !== undefined) {
        callback(tokenName, value);
      }
    });
  };
  handle.tokens.observe(handler);
  return () => handle.tokens.unobserve(handler);
}
