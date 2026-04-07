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
 * Connects to the host's WebSocket server and sets up shared CRDT types
 * for content, tokens, and comments. Returns a handle for cleanup.
 */
export function initCollabSync(
  serverUrl: string,
  roomName: string,
  identity: CollaboratorIdentity
): CollabSyncHandle {
  const ydoc = new Y.Doc();
  const provider = new WebsocketProvider(serverUrl, roomName, ydoc);

  // Set local awareness (presence)
  provider.awareness.setLocalState({
    user: identity,
    selectedSection: null,
    activePanel: null,
  } satisfies PresenceState);

  // Shared CRDT types
  const sections = ydoc.getMap<Y.Map<string>>("sections");
  const tokens = ydoc.getMap<string>("tokens");
  const comments = ydoc.getArray<any>("comments");

  const destroy = () => {
    provider.awareness.setLocalState(null);
    provider.disconnect();
    provider.destroy();
    ydoc.destroy();
  };

  return { ydoc, provider, sections, tokens, comments, destroy };
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
