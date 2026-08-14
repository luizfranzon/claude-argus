import { create } from "zustand";

import { dirname, joinPath } from "../lib/paths";
import * as tauriApi from "../lib/tauri";
import type { FileEntryDto } from "../lib/types";

interface DirState {
  entries: FileEntryDto[];
  loading: boolean;
}

interface FileTreeStoreState {
  dirs: Record<string, DirState>;
  expanded: Record<string, boolean>;

  loadDir: (path: string) => Promise<void>;
  refreshDir: (path: string) => Promise<void>;
  toggleDir: (path: string) => void;
  createEntry: (dirPath: string, name: string, isDir: boolean) => Promise<void>;
  renameEntry: (from: string, to: string) => Promise<void>;
  deleteEntry: (path: string, parentDir: string) => Promise<void>;
  movePath: (from: string, to: string, sourceParentDir: string) => Promise<void>;
}

export const useFileTreeStore = create<FileTreeStoreState>((set, get) => ({
  dirs: {},
  expanded: {},

  loadDir: async (path: string) => {
    set((state) => ({ dirs: { ...state.dirs, [path]: { entries: state.dirs[path]?.entries ?? [], loading: true } } }));
    const entries = await tauriApi.listDir(path);
    set((state) => ({ dirs: { ...state.dirs, [path]: { entries, loading: false } } }));
  },

  refreshDir: async (path: string) => {
    if (!get().dirs[path]) return;
    const entries = await tauriApi.listDir(path);
    set((state) => ({ dirs: { ...state.dirs, [path]: { entries, loading: false } } }));
  },

  toggleDir: (path: string) => {
    const isExpanded = !!get().expanded[path];
    set((state) => ({ expanded: { ...state.expanded, [path]: !isExpanded } }));
    if (!isExpanded && !get().dirs[path]) {
      void get().loadDir(path);
    }
  },

  createEntry: async (dirPath: string, name: string, isDir: boolean) => {
    const path = joinPath(dirPath, name);
    if (isDir) {
      await tauriApi.createDir(path);
    } else {
      await tauriApi.createFile(path);
    }
    await get().refreshDir(dirPath);
  },

  renameEntry: async (from: string, to: string) => {
    await tauriApi.renamePath(from, to);
    await get().refreshDir(dirname(from));
  },

  deleteEntry: async (path: string, parentDir: string) => {
    await tauriApi.deletePath(path);
    await get().refreshDir(parentDir);
  },

  movePath: async (from: string, to: string, sourceParentDir: string) => {
    await tauriApi.renamePath(from, to);
    await get().refreshDir(sourceParentDir);
    await get().refreshDir(dirname(to));
  },
}));
