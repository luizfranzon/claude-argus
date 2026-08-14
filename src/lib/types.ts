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

export type CloseDecisionDto = "RequiresConfirmation" | "AlreadyClosed";

export type WorkspaceCloseReason = "UserConfirmed" | "ProcessExited";

export interface WorkspaceClosedEvent {
  id: string;
  reason: WorkspaceCloseReason;
}

export interface StartupPathResolvedEvent {
  ok: boolean;
  error?: string;
}

export type RegionKind = "SidebarLeft" | "Grid" | "TopBar" | "BottomBar";

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

export function panelWorkspaceId(kind: PanelKind): string {
  return Object.values(kind)[0] as string;
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
