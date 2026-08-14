import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

import { useWorkspaceStore } from "../state/workspaceStore";
import type { StartupPathResolvedEvent, WorkspaceClosedEvent } from "../lib/types";

/**
 * Subscribes to the low-frequency lifecycle events emitted from Rust:
 * a workspace closing on its own (crash/exit) and startup PATH resolution
 * finishing. PTY output itself never goes through here — that's the
 * per-workspace Channel wired up in TerminalView.
 */
export function useWorkspaceEvents() {
  const removeWorkspace = useWorkspaceStore((state) => state.removeWorkspace);
  const markStartupPathResolved = useWorkspaceStore((state) => state.markStartupPathResolved);

  useEffect(() => {
    const unlistenClosed = listen<WorkspaceClosedEvent>("workspace-closed", (event) => {
      if (event.payload.reason === "ProcessExited") {
        removeWorkspace(event.payload.id);
      }
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
