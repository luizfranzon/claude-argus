import { create } from "zustand";

import {
  checkExternalChange,
  conflictResolveKeepMine,
  conflictResolveReload,
  disposeModel,
  getOrLoadModel,
  saveModel,
} from "../lib/monacoModels";
import { useWorkspaceStore } from "./workspaceStore";

export interface EditorTab {
  path: string;
  dirty: boolean;
  conflict: boolean;
}

interface EditorStoreState {
  openFiles: Record<string, EditorTab[]>;
  activeFile: Record<string, string | null>;

  openFile: (workspaceId: string, path: string) => Promise<void>;
  closeFile: (workspaceId: string, path: string) => void;
  closeWorkspace: (workspaceId: string) => void;
  setActiveFile: (workspaceId: string, path: string) => void;
  setDirty: (workspaceId: string, path: string, dirty: boolean) => void;
  saveFile: (workspaceId: string, path: string) => Promise<void>;
  checkExternalChanges: (workspaceId: string) => Promise<void>;
  resolveConflict: (workspaceId: string, path: string, keepMine: boolean) => Promise<void>;
}

function updateTab(
  tabs: Record<string, EditorTab[]>,
  workspaceId: string,
  path: string,
  patch: Partial<EditorTab>,
): Record<string, EditorTab[]> {
  return {
    ...tabs,
    [workspaceId]: (tabs[workspaceId] ?? []).map((tab) => (tab.path === path ? { ...tab, ...patch } : tab)),
  };
}

export const useEditorStore = create<EditorStoreState>((set, get) => ({
  openFiles: {},
  activeFile: {},

  openFile: async (workspaceId: string, path: string) => {
    const tabs = get().openFiles[workspaceId] ?? [];
    if (!tabs.some((tab) => tab.path === path)) {
      await getOrLoadModel(path);
      set((state) => ({
        openFiles: { ...state.openFiles, [workspaceId]: [...tabs, { path, dirty: false, conflict: false }] },
      }));
      void useWorkspaceStore.getState().openEditor(workspaceId);
    }
    set((state) => ({ activeFile: { ...state.activeFile, [workspaceId]: path } }));
  },

  closeFile: (workspaceId: string, path: string) => {
    const remaining = (get().openFiles[workspaceId] ?? []).filter((tab) => tab.path !== path);
    disposeModel(path);
    set((state) => {
      const activeFile = { ...state.activeFile };
      if (activeFile[workspaceId] === path) {
        activeFile[workspaceId] = remaining[remaining.length - 1]?.path ?? null;
      }
      return { openFiles: { ...state.openFiles, [workspaceId]: remaining }, activeFile };
    });
  },

  closeWorkspace: (workspaceId: string) => {
    for (const tab of get().openFiles[workspaceId] ?? []) {
      disposeModel(tab.path);
    }
    set((state) => {
      const { [workspaceId]: _tabs, ...openFiles } = state.openFiles;
      const { [workspaceId]: _active, ...activeFile } = state.activeFile;
      return { openFiles, activeFile };
    });
  },

  setActiveFile: (workspaceId: string, path: string) =>
    set((state) => ({ activeFile: { ...state.activeFile, [workspaceId]: path } })),

  setDirty: (workspaceId: string, path: string, dirty: boolean) =>
    set((state) => ({ openFiles: updateTab(state.openFiles, workspaceId, path, { dirty }) })),

  saveFile: async (workspaceId: string, path: string) => {
    await saveModel(path);
    set((state) => ({ openFiles: updateTab(state.openFiles, workspaceId, path, { dirty: false, conflict: false }) }));
  },

  checkExternalChanges: async (workspaceId: string) => {
    for (const tab of get().openFiles[workspaceId] ?? []) {
      const result = await checkExternalChange(tab.path);
      if (result === "reloaded") {
        set((state) => ({ openFiles: updateTab(state.openFiles, workspaceId, tab.path, { dirty: false }) }));
      } else if (result === "conflict") {
        set((state) => ({ openFiles: updateTab(state.openFiles, workspaceId, tab.path, { conflict: true }) }));
      }
    }
  },

  resolveConflict: async (workspaceId: string, path: string, keepMine: boolean) => {
    if (keepMine) {
      conflictResolveKeepMine(path);
    } else {
      await conflictResolveReload(path);
    }
    set((state) => ({
      openFiles: updateTab(state.openFiles, workspaceId, path, {
        conflict: false,
        dirty: keepMine ? state.openFiles[workspaceId]?.find((t) => t.path === path)?.dirty ?? false : false,
      }),
    }));
  },
}));
