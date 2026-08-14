import { useRef, useState, type PointerEvent, type ReactNode } from "react";

import styles from "./SplitView.module.css";

interface SplitViewProps {
  left: ReactNode;
  right: ReactNode | null;
}

/** Resizable two-pane split. Renders `left` alone (full width) when `right` is null. */
export function SplitView({ left, right }: SplitViewProps) {
  const [rightFraction, setRightFraction] = useState(0.5);
  const containerRef = useRef<HTMLDivElement>(null);
  const draggingRef = useRef(false);

  function handlePointerDown(e: PointerEvent<HTMLDivElement>) {
    draggingRef.current = true;
    e.currentTarget.setPointerCapture(e.pointerId);
  }

  function handlePointerMove(e: PointerEvent<HTMLDivElement>) {
    if (!draggingRef.current || !containerRef.current) return;
    const rect = containerRef.current.getBoundingClientRect();
    const fraction = 1 - (e.clientX - rect.left) / rect.width;
    setRightFraction(Math.min(0.8, Math.max(0.2, fraction)));
  }

  function handlePointerUp() {
    draggingRef.current = false;
  }

  if (!right) {
    return (
      <div className={styles.container} ref={containerRef}>
        <div className={styles.pane} style={{ width: "100%" }}>
          {left}
        </div>
      </div>
    );
  }

  return (
    <div className={styles.container} ref={containerRef}>
      <div className={styles.pane} style={{ width: `${(1 - rightFraction) * 100}%` }}>
        {left}
      </div>
      <div
        className={styles.handle}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
      />
      <div className={styles.pane} style={{ width: `${rightFraction * 100}%` }}>
        {right}
      </div>
    </div>
  );
}
