import { create } from "zustand";
import { Channel } from "@tauri-apps/api/core";

import * as tauriApi from "../lib/tauri";
import type { SessionDto, SessionRuntimeStatus } from "../lib/types";
import { setFraction, swapLeaves, type LayoutNode } from "../lib/gridLayout";
import { useFeatureGroupStore } from "./featureGroupStore";

/**
 * Mirrors the pending-channel-buffering pattern in `workspaceStore.ts`:
 * a Session's PTY output can start arriving over its Channel before the
 * create-session command promise resolves with the Session's id, so bytes
 * are buffered here until a `TerminalView` mounts and takes over.
 */
interface PendingChannel {
  channel: Channel<Uint8Array>;
  buffered: Uint8Array[];
}

interface SessionStoreState {
  sessions: Record<string, SessionDto>;
  sessionsByWorkspace: Record<string, string[]>;
  /** The resizable split tree over a Workspace's currently-visible Grid
   * cells. Rebuilt by `Grid.tsx` (via `setGridLayout`) whenever which
   * Sessions are visible changes; resized/swapped in place otherwise. */
  gridLayout: Record<string, LayoutNode | null>;
  /** "thinking"/"idle" per Session, from Claude Code's own hooks (see
   * `useSessionEvents`) — absent until the first hook event arrives, kept
   * separate from `SessionDto.status` (the close-confirmation lifecycle,
   * Starting/Running/.../Terminating), a different concern entirely. */
  runtimeStatus: Record<string, SessionRuntimeStatus>;
  channels: Record<string, PendingChannel>;
  pendingCloseSessionId: string | null;

  registerSession: (session: SessionDto, channel: PendingChannel) => void;
  setRuntimeStatus: (sessionId: string, status: SessionRuntimeStatus) => void;
  createSession: (workspaceId: string, name?: string) => Promise<SessionDto>;
  requestCloseSession: (id: string) => Promise<void>;
  confirmPendingClose: () => Promise<void>;
  cancelPendingClose: () => void;
  removeSession: (id: string) => void;
  removeSessionsForWorkspace: (workspaceId: string) => void;
  renameSession: (id: string, name: string) => Promise<void>;
  takeChannel: (id: string) => PendingChannel | undefined;
  setGridLayout: (workspaceId: string, node: LayoutNode | null) => void;
  resizeGridSplit: (workspaceId: string, splitId: string, fraction: number) => void;
  swapGridLeaves: (workspaceId: string, a: string, b: string) => void;
}

export function newPendingChannel(): PendingChannel {
  const pending: PendingChannel = { channel: new Channel<Uint8Array>(), buffered: [] };
  pending.channel.onmessage = (data) => {
    pending.buffered.push(data);
  };
  return pending;
}

export const useSessionStore = create<SessionStoreState>((set, get) => ({
  sessions: {},
  sessionsByWorkspace: {},
  gridLayout: {},
  runtimeStatus: {},
  channels: {},
  pendingCloseSessionId: null,

  registerSession: (session, channel) =>
    set((state) => ({
      sessions: { ...state.sessions, [session.id]: session },
      sessionsByWorkspace: {
        ...state.sessionsByWorkspace,
        [session.workspaceId]: [...(state.sessionsByWorkspace[session.workspaceId] ?? []), session.id],
      },
      channels: { ...state.channels, [session.id]: channel },
    })),

  setRuntimeStatus: (sessionId, status) =>
    set((state) => ({ runtimeStatus: { ...state.runtimeStatus, [sessionId]: status } })),

  createSession: async (workspaceId: string, name?: string) => {
    const pending = newPendingChannel();
    const session = await tauriApi.createSession(workspaceId, pending.channel, name);
    get().registerSession(session, pending);
    return session;
  },

  requestCloseSession: async (id: string) => {
    const decision = await tauriApi.requestCloseSession(id);
    if (decision === "RequiresConfirmation") {
      set({ pendingCloseSessionId: id });
    } else {
      get().removeSession(id);
    }
  },

  confirmPendingClose: async () => {
    const id = get().pendingCloseSessionId;
    if (!id) return;
    await tauriApi.confirmCloseSession(id);
    get().removeSession(id);
    set({ pendingCloseSessionId: null });
  },

  cancelPendingClose: () => set({ pendingCloseSessionId: null }),

  removeSession: (id: string) => {
    set((state) => {
      const session = state.sessions[id];
      const { [id]: _removed, ...sessions } = state.sessions;
      const { [id]: _removedChannel, ...channels } = state.channels;
      const { [id]: _removedStatus, ...runtimeStatus } = state.runtimeStatus;
      const sessionsByWorkspace = session
        ? {
            ...state.sessionsByWorkspace,
            [session.workspaceId]: (state.sessionsByWorkspace[session.workspaceId] ?? []).filter(
              (sid) => sid !== id,
            ),
          }
        : state.sessionsByWorkspace;
      return { sessions, channels, runtimeStatus, sessionsByWorkspace };
    });
    useFeatureGroupStore.getState().removeSessionFromGroups(id);
  },

  removeSessionsForWorkspace: (workspaceId: string) => {
    const ids = get().sessionsByWorkspace[workspaceId] ?? [];
    for (const id of ids) {
      get().removeSession(id);
    }
    set((state) => {
      const { [workspaceId]: _removedLayout, ...gridLayout } = state.gridLayout;
      return { gridLayout };
    });
  },

  renameSession: async (id: string, name: string) => {
    await tauriApi.renameSession(id, name);
    set((state) => ({
      sessions: { ...state.sessions, [id]: { ...state.sessions[id], name } },
    }));
  },

  takeChannel: (id: string) => get().channels[id],

  setGridLayout: (workspaceId: string, node: LayoutNode | null) =>
    set((state) => ({ gridLayout: { ...state.gridLayout, [workspaceId]: node } })),

  resizeGridSplit: (workspaceId: string, splitId: string, fraction: number) =>
    set((state) => {
      const current = state.gridLayout[workspaceId];
      if (!current) return {};
      return { gridLayout: { ...state.gridLayout, [workspaceId]: setFraction(current, splitId, fraction) } };
    }),

  swapGridLeaves: (workspaceId: string, a: string, b: string) => {
    if (a === b) return;
    set((state) => {
      const current = state.gridLayout[workspaceId];
      if (!current) return {};
      return { gridLayout: { ...state.gridLayout, [workspaceId]: swapLeaves(current, a, b) } };
    });
  },
}));
