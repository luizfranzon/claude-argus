import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

import { useWorkspaceStore } from "../state/workspaceStore";
import type { StartupPathResolvedEvent, WorkspaceClosedEvent } from "../lib/types";

/**
 * Subscribes to the low-frequency lifecycle events emitted from Rust: a
 * workspace closing and startup PATH resolution finishing. A Workspace no
 * longer owns a PTY directly (see docs/adr/0010), so it only ever closes via
 * explicit user confirmation — this listener is a safety net alongside the
 * direct `removeWorkspace` call in `confirmPendingClose`, harmless if it
 * fires twice for the same id. PTY output itself never goes through here —
 * that's the per-Session Channel wired up in TerminalView.
 */
export function useWorkspaceEvents() {
  const removeWorkspace = useWorkspaceStore((state) => state.removeWorkspace);
  const markStartupPathResolved = useWorkspaceStore((state) => state.markStartupPathResolved);

  useEffect(() => {
    const unlistenClosed = listen<WorkspaceClosedEvent>("workspace-closed", (event) => {
      removeWorkspace(event.payload.id);
    });

    const unlistenStartup = listen<StartupPathResolvedEvent>("startup-path-resolved", (event) => {
      markStartupPathResolved(event.payload.ok, event.payload.error);
    });

    return () => {
      unlistenClosed.then((fn) => fn());
      unlistenStartup.then((fn) => fn());
    };
  }, [removeWorkspace, markStartupPathResolved]);
}
