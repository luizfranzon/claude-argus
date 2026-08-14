export type WorkspaceStatus =
  | "Starting"
  | "Running"
  | "AwaitingCloseConfirmation"
  | "Terminating";

export interface WorkspaceDto {
  id: string;
  directory: string;
  status: WorkspaceStatus;
}

export type SessionStatus = "Starting" | "Running" | "AwaitingCloseConfirmation" | "Terminating";

export interface SessionDto {
  id: string;
  workspaceId: string;
  name: string;
  status: SessionStatus;
}

/// What every Workspace-creating command returns: the Workspace plus the
/// first Session auto-spawned inside it (see CONTEXT.md's "Session").
export interface CreateWorkspaceResponse {
  workspace: WorkspaceDto;
  session: SessionDto;
}

export type CloseDecisionDto = "RequiresConfirmation" | "AlreadyClosed";

// A Workspace no longer owns a PTY directly (see docs/adr/0010), so it is
// only ever closed by explicit user action — compare SessionCloseReason,
// which can also be "ProcessExited".
export type WorkspaceCloseReason = "UserConfirmed";

export interface WorkspaceClosedEvent {
  id: string;
  reason: WorkspaceCloseReason;
}

export type SessionCloseReason = "UserConfirmed" | "ProcessExited";

/// Whether a Session's `claude` process is actively working on a prompt —
/// derived from Claude Code's own `UserPromptSubmit`/`Stop` hooks (see
/// docs/adr/0010), not from parsing PTY output.
export type SessionRuntimeStatus = "thinking" | "idle";

export interface SessionStatusChangedEvent {
  sessionId: string;
  status: SessionRuntimeStatus;
}

export interface SessionClosedEvent {
  id: string;
  reason: SessionCloseReason;
}

export interface StartupPathResolvedEvent {
  ok: boolean;
  error?: string;
}

export type RegionKind = "SidebarLeft" | "Grid" | "TopBar" | "BottomBar";

// `Terminal`'s payload is a SessionId as of v3 (see docs/adr/0010) — Editor/
// FileExplorer/GitPanel stay WorkspaceId-scoped.
export type PanelKind =
  | { Terminal: string }
  | { Editor: string }
  | { FileExplorer: string }
  | { GitPanel: string };

export interface PanelDto {
  id: string;
  kind: PanelKind;
  region: RegionKind;
}

export interface RegionDto {
  kind: RegionKind;
  panels: string[];
}

export interface ShellLayoutDto {
  regions: RegionDto[];
  panels: PanelDto[];
}

export type PanelOwner = { kind: "workspace"; id: string } | { kind: "session"; id: string };

export function panelOwner(kind: PanelKind): PanelOwner {
  if ("Terminal" in kind) {
    return { kind: "session", id: kind.Terminal };
  }
  return { kind: "workspace", id: Object.values(kind)[0] as string };
}

export interface FileEntryDto {
  name: string;
  path: string;
  isDir: boolean;
}

export interface FsChangedEvent {
  workspaceId: string;
}

export interface GitRepositoryDto {
  name: string;
  path: string;
  isSubmodule: boolean;
}

export type FileStatusKind = "Modified" | "Added" | "Deleted" | "Renamed" | "Untracked" | "Conflicted";

export interface FileStatusEntryDto {
  path: string;
  staged: boolean;
  kind: FileStatusKind;
}

export interface DiffContentDto {
  old: string;
  new: string;
}

export interface CommitEntryDto {
  hash: string;
  shortHash: string;
  author: string;
  date: string;
  summary: string;
}

export interface BranchInfoDto {
  name: string;
  isCurrent: boolean;
}

export interface SyncStatusDto {
  ahead: number;
  behind: number;
  hasUpstream: boolean;
}
