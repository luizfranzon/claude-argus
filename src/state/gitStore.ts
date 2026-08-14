import { create } from "zustand";

import * as tauriApi from "../lib/tauri";
import type { BranchInfoDto, CommitEntryDto, FileStatusEntryDto, GitRepositoryDto, SyncStatusDto } from "../lib/types";

const HISTORY_PAGE_SIZE = 30;

export interface DiffTarget {
  repoPath: string;
  file: string;
  staged: boolean;
}

interface GitStoreState {
  gitAvailable: Record<string, boolean>;
  repositories: Record<string, GitRepositoryDto[]>;
  statuses: Record<string, FileStatusEntryDto[]>;
  branches: Record<string, BranchInfoDto[]>;
  currentBranch: Record<string, string | null>;
  sync: Record<string, SyncStatusDto>;
  commitMessage: Record<string, string>;
  history: Record<string, CommitEntryDto[]>;
  historyComplete: Record<string, boolean>;
  activeDiff: DiffTarget | null;

  loadWorkspace: (workspaceId: string, workspaceRoot: string) => Promise<void>;
  refreshRepo: (repoPath: string) => Promise<void>;
  stageFiles: (repoPath: string, files: string[]) => Promise<void>;
  unstageFiles: (repoPath: string, files: string[]) => Promise<void>;
  setCommitMessage: (repoPath: string, message: string) => void;
  commit: (repoPath: string) => Promise<void>;
  loadMoreHistory: (repoPath: string) => Promise<void>;
  switchBranch: (repoPath: string, name: string) => Promise<void>;
  push: (repoPath: string) => Promise<void>;
  pull: (repoPath: string) => Promise<void>;
  fetch: (repoPath: string) => Promise<void>;
  openDiff: (target: DiffTarget) => void;
  closeDiff: () => void;
}

export const useGitStore = create<GitStoreState>((set, get) => ({
  gitAvailable: {},
  repositories: {},
  statuses: {},
  branches: {},
  currentBranch: {},
  sync: {},
  commitMessage: {},
  history: {},
  historyComplete: {},
  activeDiff: null,

  loadWorkspace: async (workspaceId: string, workspaceRoot: string) => {
    const available = await tauriApi.gitAvailable();
    set((state) => ({ gitAvailable: { ...state.gitAvailable, [workspaceId]: available } }));
    if (!available) return;

    const repos = await tauriApi.gitListRepositories(workspaceRoot);
    set((state) => ({ repositories: { ...state.repositories, [workspaceId]: repos } }));
    await Promise.all(repos.map((repo) => get().refreshRepo(repo.path)));
  },

  refreshRepo: async (repoPath: string) => {
    // `allSettled`, not `all` — an empty repo (no commits yet) or a repo
    // mid-operation can make one of these fail; the rest should still land.
    const [status, branches, currentBranch, sync] = await Promise.allSettled([
      tauriApi.gitStatus(repoPath),
      tauriApi.gitListBranches(repoPath),
      tauriApi.gitCurrentBranch(repoPath),
      tauriApi.gitSyncStatus(repoPath),
    ]);
    set((state) => ({
      statuses: status.status === "fulfilled" ? { ...state.statuses, [repoPath]: status.value } : state.statuses,
      branches: branches.status === "fulfilled" ? { ...state.branches, [repoPath]: branches.value } : state.branches,
      currentBranch:
        currentBranch.status === "fulfilled"
          ? { ...state.currentBranch, [repoPath]: currentBranch.value }
          : state.currentBranch,
      sync: sync.status === "fulfilled" ? { ...state.sync, [repoPath]: sync.value } : state.sync,
    }));
  },

  stageFiles: async (repoPath: string, files: string[]) => {
    await tauriApi.gitStage(repoPath, files);
    await get().refreshRepo(repoPath);
  },

  unstageFiles: async (repoPath: string, files: string[]) => {
    await tauriApi.gitUnstage(repoPath, files);
    await get().refreshRepo(repoPath);
  },

  setCommitMessage: (repoPath: string, message: string) =>
    set((state) => ({ commitMessage: { ...state.commitMessage, [repoPath]: message } })),

  commit: async (repoPath: string) => {
    const message = get().commitMessage[repoPath];
    if (!message?.trim()) return;
    await tauriApi.gitCommit(repoPath, message.trim());
    set((state) => ({ commitMessage: { ...state.commitMessage, [repoPath]: "" } }));
    await get().refreshRepo(repoPath);
    const { [repoPath]: _history, ...history } = get().history;
    set({ history });
  },

  loadMoreHistory: async (repoPath: string) => {
    const current = get().history[repoPath] ?? [];
    const page = await tauriApi.gitLog(repoPath, current.length, HISTORY_PAGE_SIZE);
    set((state) => ({
      history: { ...state.history, [repoPath]: [...current, ...page] },
      historyComplete: { ...state.historyComplete, [repoPath]: page.length < HISTORY_PAGE_SIZE },
    }));
  },

  switchBranch: async (repoPath: string, name: string) => {
    await tauriApi.gitSwitchBranch(repoPath, name);
    await get().refreshRepo(repoPath);
  },

  push: async (repoPath: string) => {
    await tauriApi.gitPush(repoPath);
    await get().refreshRepo(repoPath);
  },

  pull: async (repoPath: string) => {
    await tauriApi.gitPull(repoPath);
    await get().refreshRepo(repoPath);
  },

  fetch: async (repoPath: string) => {
    await tauriApi.gitFetch(repoPath);
    await get().refreshRepo(repoPath);
  },

  openDiff: (target: DiffTarget) => set({ activeDiff: target }),
  closeDiff: () => set({ activeDiff: null }),
}));
