import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";

import { useSessionStore } from "../../state/sessionStore";
import * as tauriApi from "../../lib/tauri";
import styles from "./TerminalView.module.css";

interface TerminalViewProps {
  sessionId: string;
  active: boolean;
}

function readToken(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/**
 * One xterm.js instance per Session, kept mounted (just hidden) once
 * created so switching tabs never tears down or re-buffers PTY output.
 * On mount it takes over the Session's Channel from the store, flushing
 * whatever arrived before this component existed.
 */
export function TerminalView({ sessionId, active }: TerminalViewProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const term = new Terminal({
      cursorBlink: true,
      fontFamily: readToken("--font-mono") || "monospace",
      theme: {
        background: readToken("--bg"),
        foreground: readToken("--fg"),
        cursor: readToken("--fg"),
        selectionBackground: readToken("--accent-soft"),
      },
    });
    const fitAddon = new FitAddon();
    termRef.current = term;
    fitAddonRef.current = fitAddon;
    term.loadAddon(fitAddon);
    term.open(container);
    fitAddon.fit();

    const pending = useSessionStore.getState().takeChannel(sessionId);
    if (pending) {
      for (const chunk of pending.buffered) {
        term.write(chunk);
      }
      pending.buffered = [];
      pending.channel.onmessage = (data) => term.write(data);
    }

    const dataDisposable = term.onData((data) => {
      void tauriApi.writeToPty(sessionId, new TextEncoder().encode(data));
    });

    const resizeObserver = new ResizeObserver(() => {
      // A hidden (display:none) container reports a zero-size box; fitting
      // against that would corrupt the terminal's cell metrics, so only
      // react to resizes while actually visible.
      if (container.offsetParent === null) return;
      fitAddon.fit();
      void tauriApi.resizePty(sessionId, term.cols, term.rows);
    });
    resizeObserver.observe(container);
    void tauriApi.resizePty(sessionId, term.cols, term.rows);

    // Native listeners in the capture phase, not React's onDragOver/onDrop
    // props — xterm.js registers its own drag/drop listeners on this same
    // container (for dragging out selected text) and calls stopPropagation,
    // which would stop a bubble-phase React handler from ever firing. A
    // capture-phase listener runs before that, so it isn't affected.
    function handleDragOver(e: globalThis.DragEvent) {
      if (e.dataTransfer?.types.includes("text/plain")) {
        e.preventDefault();
        e.dataTransfer.dropEffect = "copy";
      }
    }
    function handleDrop(e: globalThis.DragEvent) {
      const text = e.dataTransfer?.getData("text/plain");
      if (!text) return;
      e.preventDefault();
      // Inserts at the cursor via xterm's own paste handling — same path as
      // a real clipboard paste, never executed automatically. See "Path
      // reference" in CONTEXT.md. Padded with spaces so it doesn't run into
      // whatever's already at the cursor on either side.
      term.paste(` ${text} `);
    }
    container.addEventListener("dragover", handleDragOver, true);
    container.addEventListener("drop", handleDrop, true);

    return () => {
      dataDisposable.dispose();
      resizeObserver.disconnect();
      container.removeEventListener("dragover", handleDragOver, true);
      container.removeEventListener("drop", handleDrop, true);
      term.dispose();
      termRef.current = null;
      fitAddonRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId]);

  // Switching back to a tab that was hidden needs an explicit re-fit: xterm's
  // cell measurements can go stale while its container had zero size, and
  // toggling `display` alone doesn't force it to remeasure — only an actual
  // resize (or this) does.
  useEffect(() => {
    if (!active) return;
    const term = termRef.current;
    const fitAddon = fitAddonRef.current;
    if (!term || !fitAddon) return;
    const raf = requestAnimationFrame(() => {
      fitAddon.fit();
      void tauriApi.resizePty(sessionId, term.cols, term.rows);
    });
    return () => cancelAnimationFrame(raf);
  }, [active, sessionId]);

  return (
    <div ref={containerRef} className={styles.container} style={{ display: active ? "block" : "none" }} />
  );
}
