import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

import { useSessionStore } from "../state/sessionStore";
import type { SessionClosedEvent, SessionStatusChangedEvent } from "../lib/types";

/**
 * Subscribes to Session lifecycle/status events emitted from Rust: a
 * Session's `claude` process ending on its own (crash/exit), and its
 * "thinking"/"idle" status from Claude Code's own hooks (see
 * docs/adr/0010). Mirrors `useWorkspaceEvents`'s pattern, scoped to
 * Sessions instead — unlike a Workspace, a Session's own PTY can still
 * exit unprompted, so this is the only place that removal actually happens.
 */
export function useSessionEvents() {
  const removeSession = useSessionStore((state) => state.removeSession);
  const setRuntimeStatus = useSessionStore((state) => state.setRuntimeStatus);

  useEffect(() => {
    const unlistenClosed = listen<SessionClosedEvent>("session-closed", (event) => {
      removeSession(event.payload.id);
    });

    const unlistenStatus = listen<SessionStatusChangedEvent>("session-status-changed", (event) => {
      setRuntimeStatus(event.payload.sessionId, event.payload.status);
    });

    return () => {
      unlistenClosed.then((fn) => fn());
      unlistenStatus.then((fn) => fn());
    };
  }, [removeSession, setRuntimeStatus]);
}
