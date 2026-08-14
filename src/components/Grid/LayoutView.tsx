import type { LayoutNode } from "../../lib/gridLayout";
import type { SessionDto } from "../../lib/types";
import { GridCell } from "./GridCell";
import { ResizableSplit } from "./ResizableSplit";

interface LayoutViewProps {
  node: LayoutNode;
  sessions: Record<string, SessionDto>;
  draggingId: string | null;
  overId: string | null;
  onRequestClose: (sessionId: string) => void;
  onResize: (splitId: string, fraction: number) => void;
}

/** Renders a resizable split tree recursively — each leaf is a Session's
 * Grid cell, each split a draggable divider between two subtrees. */
export function LayoutView({ node, sessions, draggingId, overId, onRequestClose, onResize }: LayoutViewProps) {
  if (node.type === "leaf") {
    const session = sessions[node.sessionId];
    if (!session) return null;
    return (
      <GridCell
        session={session}
        visible
        isDragging={draggingId === session.id}
        isDropTarget={overId === session.id && draggingId !== session.id}
        onRequestClose={onRequestClose}
      />
    );
  }

  return (
    <ResizableSplit
      direction={node.direction}
      fraction={node.fraction}
      onFractionChange={(fraction) => onResize(node.id, fraction)}
      a={
        <LayoutView
          node={node.a}
          sessions={sessions}
          draggingId={draggingId}
          overId={overId}
          onRequestClose={onRequestClose}
          onResize={onResize}
        />
      }
      b={
        <LayoutView
          node={node.b}
          sessions={sessions}
          draggingId={draggingId}
          overId={overId}
          onRequestClose={onRequestClose}
          onResize={onResize}
        />
      }
    />
  );
}
