import { invoke } from "@tauri-apps/api/core";
import { Channel } from "@tauri-apps/api/core";

import type {
  BranchInfoDto,
  CloseDecisionDto,
  CommitEntryDto,
  CreateWorkspaceResponse,
  DiffContentDto,
  FileEntryDto,
  FileStatusEntryDto,
  GitRepositoryDto,
  SessionDto,
  ShellLayoutDto,
  SyncStatusDto,
  WorkspaceDto,
} from "./types";

export function resolveStartupPath(): Promise<void> {
  return invoke("resolve_startup_path");
}

export function getInitialDirectory(): Promise<string | null> {
  return invoke("get_initial_directory");
}

export function createWorkspaceViaPicker(
  channel: Channel<Uint8Array>,
): Promise<CreateWorkspaceResponse | null> {
  return invoke("create_workspace_via_picker", { channel });
}

export function createWorkspaceWithDirectory(
  directory: string,
  channel: Channel<Uint8Array>,
): Promise<CreateWorkspaceResponse> {
  return invoke("create_workspace_with_directory", { channel, directory });
}

export function duplicateWorkspace(
  sourceId: string,
  channel: Channel<Uint8Array>,
): Promise<CreateWorkspaceResponse | null> {
  return invoke("duplicate_workspace", { channel, sourceId });
}

export function requestCloseWorkspace(id: string): Promise<CloseDecisionDto> {
  return invoke("request_close_workspace", { id });
}

export function confirmCloseWorkspace(id: string): Promise<void> {
  return invoke("confirm_close_workspace", { id });
}

export function createSession(
  workspaceId: string,
  channel: Channel<Uint8Array>,
  name?: string,
): Promise<SessionDto> {
  return invoke("create_session", { workspaceId, channel, name: name ?? null });
}

export function requestCloseSession(id: string): Promise<CloseDecisionDto> {
  return invoke("request_close_session", { id });
}

export function confirmCloseSession(id: string): Promise<void> {
  return invoke("confirm_close_session", { id });
}

export function renameSession(id: string, name: string): Promise<void> {
  return invoke("rename_session", { id, name });
}

export function listSessions(): Promise<SessionDto[]> {
  return invoke("list_sessions");
}

/** `id` is a SessionId — a Workspace no longer owns a PTY directly (ADR-0010). */
export function writeToPty(id: string, data: Uint8Array): Promise<void> {
  return invoke("write_to_pty", { id, data: Array.from(data) });
}

/** `id` is a SessionId — a Workspace no longer owns a PTY directly (ADR-0010). */
export function resizePty(id: string, cols: number, rows: number): Promise<void> {
  return invoke("resize_pty", { id, cols, rows });
}

export function listWorkspaces(): Promise<WorkspaceDto[]> {
  return invoke("list_workspaces");
}

export function getShellLayout(): Promise<ShellLayoutDto> {
  return invoke("get_shell_layout");
}

export function openEditorPanel(id: string): Promise<ShellLayoutDto> {
  return invoke("open_editor_panel", { id });
}

export function listDir(path: string): Promise<FileEntryDto[]> {
  return invoke("list_dir", { path });
}

export function readFile(path: string): Promise<string> {
  return invoke("read_file", { path });
}

export function writeFile(path: string, contents: string): Promise<void> {
  return invoke("write_file", { path, contents });
}

export function createFile(path: string): Promise<void> {
  return invoke("create_file", { path });
}

export function createDir(path: string): Promise<void> {
  return invoke("create_dir", { path });
}

export function renamePath(from: string, to: string): Promise<void> {
  return invoke("rename_path", { from, to });
}

export function deletePath(path: string): Promise<void> {
  return invoke("delete_path", { path });
}

export function gitAvailable(): Promise<boolean> {
  return invoke("git_available");
}

export function gitListRepositories(workspaceRoot: string): Promise<GitRepositoryDto[]> {
  return invoke("git_list_repositories", { workspaceRoot });
}

export function gitStatus(repoPath: string): Promise<FileStatusEntryDto[]> {
  return invoke("git_status", { repoPath });
}

export function gitDiff(repoPath: string, file: string, staged: boolean): Promise<DiffContentDto> {
  return invoke("git_diff", { repoPath, file, staged });
}

export function gitStage(repoPath: string, files: string[]): Promise<void> {
  return invoke("git_stage", { repoPath, files });
}

export function gitUnstage(repoPath: string, files: string[]): Promise<void> {
  return invoke("git_unstage", { repoPath, files });
}

export function gitCommit(repoPath: string, message: string): Promise<void> {
  return invoke("git_commit", { repoPath, message });
}

export function gitLog(repoPath: string, skip: number, limit: number): Promise<CommitEntryDto[]> {
  return invoke("git_log", { repoPath, skip, limit });
}

export function gitCurrentBranch(repoPath: string): Promise<string | null> {
  return invoke("git_current_branch", { repoPath });
}

export function gitListBranches(repoPath: string): Promise<BranchInfoDto[]> {
  return invoke("git_list_branches", { repoPath });
}

export function gitSwitchBranch(repoPath: string, name: string): Promise<void> {
  return invoke("git_switch_branch", { repoPath, name });
}

export function gitSyncStatus(repoPath: string): Promise<SyncStatusDto> {
  return invoke("git_sync_status", { repoPath });
}

export function gitPush(repoPath: string): Promise<void> {
  return invoke("git_push", { repoPath });
}

export function gitPull(repoPath: string): Promise<void> {
  return invoke("git_pull", { repoPath });
}

export function gitFetch(repoPath: string): Promise<void> {
  return invoke("git_fetch", { repoPath });
}

export { Channel };
