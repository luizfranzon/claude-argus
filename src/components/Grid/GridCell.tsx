import { GripVertical, X } from "lucide-react";
import { useDraggable, useDroppable } from "@dnd-kit/core";

import { TerminalView } from "../TerminalView/TerminalView";
import { useSessionStore } from "../../state/sessionStore";
import type { SessionDto } from "../../lib/types";
import styles from "./Grid.module.css";

interface GridCellProps {
  session: SessionDto;
  visible: boolean;
  isDragging: boolean;
  isDropTarget: boolean;
  onRequestClose: (sessionId: string) => void;
}

/**
 * One Grid cell: a Session's Terminal plus its chrome (name, close, drag
 * handle). The drag handle is a sibling of the Terminal's container, not a
 * wrapper around it — xterm.js registers its own capture-phase native drag
 * listeners on its container, which would otherwise fight dnd-kit's sensors.
 */
export function GridCell({ session, visible, isDragging, isDropTarget, onRequestClose }: GridCellProps) {
  const { attributes, listeners, setNodeRef: setDragRef } = useDraggable({ id: session.id });
  const { setNodeRef: setDropRef } = useDroppable({ id: session.id });
  const runtimeStatus = useSessionStore((state) => state.runtimeStatus[session.id]);

  return (
    <div
      ref={setDropRef}
      className={[styles.cell, isDragging && styles.cellDragging, isDropTarget && styles.cellDropTarget]
        .filter(Boolean)
        .join(" ")}
      style={{ display: visible ? "flex" : "none" }}
    >
      <div className={styles.cellHeader}>
        <button
          type="button"
          ref={setDragRef}
          className={styles.dragHandle}
          title="Drag to reposition"
          {...listeners}
          {...attributes}
        >
          <GripVertical size={13} />
        </button>
        <span className={styles.cellName}>{session.name}</span>
        <span
          className={
            runtimeStatus === "thinking" ? `${styles.cellStatusDot} ${styles.cellStatusThinking}` : styles.cellStatusDot
          }
          title={runtimeStatus === "thinking" ? "Thinking" : runtimeStatus === "idle" ? "Idle" : "Status unknown"}
        />
        <button
          type="button"
          className={styles.cellCloseButton}
          title="Close session"
          onClick={() => onRequestClose(session.id)}
        >
          <X size={13} />
        </button>
      </div>
      <div className={styles.cellBody}>
        <TerminalView sessionId={session.id} active={visible} />
      </div>
    </div>
  );
}
