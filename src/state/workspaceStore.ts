import { create } from "zustand";

import * as tauriApi from "../lib/tauri";
import type { CreateWorkspaceResponse, ShellLayoutDto, WorkspaceDto } from "../lib/types";
import { useEditorStore } from "./editorStore";
import { useFeatureGroupStore } from "./featureGroupStore";
import { newPendingChannel, useSessionStore } from "./sessionStore";

export type SidebarTab = "agents" | "explorer" | "git";

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
  refreshShellLayout: () => Promise<void>;
  setSidebarTab: (workspaceId: string, tab: SidebarTab) => void;
  toggleSidebarCollapsed: (workspaceId: string) => void;
  openEditor: (workspaceId: string) => Promise<void>;
}

/** Registers the created Workspace and its auto-spawned first Session. */
function registerWorkspace(
  set: (fn: (state: WorkspaceStoreState) => Partial<WorkspaceStoreState>) => void,
  created: CreateWorkspaceResponse,
  pending: ReturnType<typeof newPendingChannel>,
) {
  const { workspace, session } = created;
  set((state) => ({
    workspaces: { ...state.workspaces, [workspace.id]: workspace },
    order: [...state.order, workspace.id],
    activeWorkspaceId: workspace.id,
    sidebar: { ...state.sidebar, [workspace.id]: defaultSidebarState() },
  }));
  useSessionStore.getState().registerSession(session, pending);
}

export const useWorkspaceStore = create<WorkspaceStoreState>((set, get) => ({
  workspaces: {},
  order: [],
  activeWorkspaceId: null,
  startupPathStatus: "pending",
  startupPathError: undefined,
  pendingCloseWorkspaceId: null,
  shellLayout: null,
  sidebar: {},

  createWorkspaceViaPicker: async () => {
    const pending = newPendingChannel();
    const created = await tauriApi.createWorkspaceViaPicker(pending.channel);
    if (created) {
      registerWorkspace(set, created, pending);
      void get().refreshShellLayout();
    }
  },

  createWorkspaceWithDirectory: async (directory: string) => {
    const pending = newPendingChannel();
    const created = await tauriApi.createWorkspaceWithDirectory(directory, pending.channel);
    registerWorkspace(set, created, pending);
    void get().refreshShellLayout();
  },

  duplicateWorkspace: async (sourceId: string) => {
    const pending = newPendingChannel();
    const created = await tauriApi.duplicateWorkspace(sourceId, pending.channel);
    if (created) {
      registerWorkspace(set, created, pending);
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
      const { [id]: _removedSidebar, ...sidebar } = state.sidebar;
      const order = state.order.filter((wid) => wid !== id);
      const activeWorkspaceId =
        state.activeWorkspaceId === id ? (order[order.length - 1] ?? null) : state.activeWorkspaceId;
      return { workspaces, sidebar, order, activeWorkspaceId };
    });
    useSessionStore.getState().removeSessionsForWorkspace(id);
    useFeatureGroupStore.getState().removeGroupsForWorkspace(id);
    useEditorStore.getState().closeWorkspace(id);
    void get().refreshShellLayout();
  },

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
