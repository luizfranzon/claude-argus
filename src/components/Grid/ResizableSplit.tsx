import { useRef, type PointerEvent, type ReactNode } from "react";

import styles from "./ResizableSplit.module.css";

interface ResizableSplitProps {
  direction: "row" | "column";
  fraction: number;
  onFractionChange: (fraction: number) => void;
  a: ReactNode;
  b: ReactNode;
}

/** Two resizable panes, tmux-style — same pointer-capture-drag mechanics as
 * `SplitView`, generalized to either axis so it can nest into a tree. */
export function ResizableSplit({ direction, fraction, onFractionChange, a, b }: ResizableSplitProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const draggingRef = useRef(false);

  function handlePointerDown(e: PointerEvent<HTMLDivElement>) {
    draggingRef.current = true;
    e.currentTarget.setPointerCapture(e.pointerId);
  }

  function handlePointerMove(e: PointerEvent<HTMLDivElement>) {
    if (!draggingRef.current || !containerRef.current) return;
    const rect = containerRef.current.getBoundingClientRect();
    const raw =
      direction === "row" ? (e.clientX - rect.left) / rect.width : (e.clientY - rect.top) / rect.height;
    onFractionChange(Math.min(0.85, Math.max(0.15, raw)));
  }

  function handlePointerUp() {
    draggingRef.current = false;
  }

  return (
    <div ref={containerRef} className={direction === "row" ? styles.row : styles.column}>
      <div className={styles.pane} style={{ flexBasis: `${fraction * 100}%` }}>
        {a}
      </div>
      <div
        className={direction === "row" ? styles.handleRow : styles.handleColumn}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
      />
      <div className={styles.pane} style={{ flexBasis: `${(1 - fraction) * 100}%` }}>
        {b}
      </div>
    </div>
  );
}
