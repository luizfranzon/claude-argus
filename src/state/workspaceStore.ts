import { create } from "zustand";
import { Channel } from "@tauri-apps/api/core";

import * as tauriApi from "../lib/tauri";
import type { ShellLayoutDto, WorkspaceDto } from "../lib/types";
import { useEditorStore } from "./editorStore";

/**
 * Output bytes can arrive on a workspace's Channel before the create-command
 * promise resolves with its WorkspaceId (the PTY starts producing output
 * as soon as it spawns, not when the Rust use case returns). Until a
 * `TerminalView` mounts and takes over `channel.onmessage`, buffer bytes
 * here so nothing written before mount is lost.
 */
interface PendingChannel {
  channel: Channel<Uint8Array>;
  buffered: Uint8Array[];
}

export type SidebarTab = "explorer" | "git";

export interface SidebarState {
  activeTab: SidebarTab;
  collapsed: boolean;
}

function defaultSidebarState(): SidebarState {
  return { activeTab: "explorer", collapsed: false };
}

interface WorkspaceStoreState {
  workspaces: Record<string, WorkspaceDto>;
  order: string[];
  activeWorkspaceId: string | null;
  startupPathStatus: "pending" | "ready" | "error";
  startupPathError?: string;
  pendingCloseWorkspaceId: string | null;
  channels: Record<string, PendingChannel>;
  shellLayout: ShellLayoutDto | null;
  sidebar: Record<string, SidebarState>;

  createWorkspaceViaPicker: () => Promise<void>;
  createWorkspaceWithDirectory: (directory: string) => Promise<void>;
  duplicateWorkspace: (sourceId: string) => Promise<void>;
  requestCloseWorkspace: (id: string) => Promise<void>;
  confirmPendingClose: () => Promise<void>;
  cancelPendingClose: () => void;
  setActiveWorkspace: (id: string) => void;
  markStartupPathResolved: (ok: boolean, error?: string) => void;
  removeWorkspace: (id: string) => void;
  takeChannel: (id: string) => PendingChannel | undefined;
  refreshShellLayout: () => Promise<void>;
  setSidebarTab: (workspaceId: string, tab: SidebarTab) => void;
  toggleSidebarCollapsed: (workspaceId: string) => void;
  openEditor: (workspaceId: string) => Promise<void>;
}

function newPendingChannel(): PendingChannel {
  const pending: PendingChannel = { channel: new Channel<Uint8Array>(), buffered: [] };
  pending.channel.onmessage = (data) => {
    pending.buffered.push(data);
  };
  return pending;
}

function registerWorkspace(
  set: (fn: (state: WorkspaceStoreState) => Partial<WorkspaceStoreState>) => void,
  workspace: WorkspaceDto,
  pending: PendingChannel,
) {
  set((state) => ({
    workspaces: { ...state.workspaces, [workspace.id]: workspace },
    order: [...state.order, workspace.id],
    activeWorkspaceId: workspace.id,
    channels: { ...state.channels, [workspace.id]: pending },
    sidebar: { ...state.sidebar, [workspace.id]: defaultSidebarState() },
  }));
}

export const useWorkspaceStore = create<WorkspaceStoreState>((set, get) => ({
  workspaces: {},
  order: [],
  activeWorkspaceId: null,
  startupPathStatus: "pending",
  startupPathError: undefined,
  pendingCloseWorkspaceId: null,
  channels: {},
  shellLayout: null,
  sidebar: {},

  createWorkspaceViaPicker: async () => {
    const pending = newPendingChannel();
    const workspace = await tauriApi.createWorkspaceViaPicker(pending.channel);
    if (workspace) {
      registerWorkspace(set, workspace, pending);
      void get().refreshShellLayout();
    }
  },

  createWorkspaceWithDirectory: async (directory: string) => {
    const pending = newPendingChannel();
    const workspace = await tauriApi.createWorkspaceWithDirectory(directory, pending.channel);
    registerWorkspace(set, workspace, pending);
    void get().refreshShellLayout();
  },

  duplicateWorkspace: async (sourceId: string) => {
    const pending = newPendingChannel();
    const workspace = await tauriApi.duplicateWorkspace(sourceId, pending.channel);
    if (workspace) {
      registerWorkspace(set, workspace, pending);
      void get().refreshShellLayout();
    }
  },

  requestCloseWorkspace: async (id: string) => {
    const decision = await tauriApi.requestCloseWorkspace(id);
    if (decision === "RequiresConfirmation") {
      set({ pendingCloseWorkspaceId: id });
    } else {
      get().removeWorkspace(id);
    }
  },

  confirmPendingClose: async () => {
    const id = get().pendingCloseWorkspaceId;
    if (!id) return;
    await tauriApi.confirmCloseWorkspace(id);
    get().removeWorkspace(id);
    set({ pendingCloseWorkspaceId: null });
  },

  cancelPendingClose: () => set({ pendingCloseWorkspaceId: null }),

  setActiveWorkspace: (id: string) => set({ activeWorkspaceId: id }),

  markStartupPathResolved: (ok: boolean, error?: string) =>
    set({ startupPathStatus: ok ? "ready" : "error", startupPathError: error }),

  removeWorkspace: (id: string) => {
    set((state) => {
      const { [id]: _removed, ...workspaces } = state.workspaces;
      const { [id]: _removedChannel, ...channels } = state.channels;
      const { [id]: _removedSidebar, ...sidebar } = state.sidebar;
      const order = state.order.filter((wid) => wid !== id);
      const activeWorkspaceId =
        state.activeWorkspaceId === id ? (order[order.length - 1] ?? null) : state.activeWorkspaceId;
      return { workspaces, channels, sidebar, order, activeWorkspaceId };
    });
    useEditorStore.getState().closeWorkspace(id);
    void get().refreshShellLayout();
  },

  takeChannel: (id: string) => get().channels[id],

  refreshShellLayout: async () => {
    const shellLayout = await tauriApi.getShellLayout();
    set({ shellLayout });
  },

  setSidebarTab: (workspaceId: string, tab: SidebarTab) =>
    set((state) => ({
      sidebar: {
        ...state.sidebar,
        [workspaceId]: { ...(state.sidebar[workspaceId] ?? defaultSidebarState()), activeTab: tab },
      },
    })),

  toggleSidebarCollapsed: (workspaceId: string) =>
    set((state) => {
      const current = state.sidebar[workspaceId] ?? defaultSidebarState();
      return {
        sidebar: {
          ...state.sidebar,
          [workspaceId]: { ...current, collapsed: !current.collapsed },
        },
      };
    }),

  openEditor: async (workspaceId: string) => {
    const shellLayout = await tauriApi.openEditorPanel(workspaceId);
    set({ shellLayout });
  },
}));
