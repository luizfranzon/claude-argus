import { useEffect, useState } from "react";
import { DndContext, PointerSensor, useSensor, useSensors, type DragEndEvent } from "@dnd-kit/core";

import { ALL_FILTER, groupFor, useFeatureGroupStore } from "../../state/featureGroupStore";
import { useSessionStore } from "../../state/sessionStore";
import { useWorkspaceStore } from "../../state/workspaceStore";
import { buildBalancedLayout, sameIdSet, collectLeafIds } from "../../lib/gridLayout";
import { GridCell } from "./GridCell";
import { LayoutView } from "./LayoutView";
import styles from "./Grid.module.css";

/**
 * The main terminal area: every Session of the active Workspace that passes
 * the Session Filter, tiled as a resizable (tmux-style) split tree. Dragging
 * one cell onto another swaps which Session occupies which leaf. Sessions
 * that don't currently show — filtered out, or belonging to another
 * Workspace — stay mounted (never unmounted, same reasoning as today's
 * Workspace tabs) but hidden via CSS so their PTY output keeps flowing.
 */
export function Grid() {
  const order = useWorkspaceStore((state) => state.order);
  const activeWorkspaceId = useWorkspaceStore((state) => state.activeWorkspaceId);
  const sessions = useSessionStore((state) => state.sessions);
  const sessionsByWorkspace = useSessionStore((state) => state.sessionsByWorkspace);
  const gridLayoutMap = useSessionStore((state) => state.gridLayout);
  const setGridLayout = useSessionStore((state) => state.setGridLayout);
  const resizeGridSplit = useSessionStore((state) => state.resizeGridSplit);
  const swapGridLeaves = useSessionStore((state) => state.swapGridLeaves);
  const requestCloseSession = useSessionStore((state) => state.requestCloseSession);
  const sessionGroup = useFeatureGroupStore((state) => state.sessionGroup);
  const sessionFilter = useFeatureGroupStore((state) =>
    activeWorkspaceId ? (state.sessionFilter[activeWorkspaceId] ?? ALL_FILTER) : ALL_FILTER,
  );

  const [draggingId, setDraggingId] = useState<string | null>(null);
  const [overId, setOverId] = useState<string | null>(null);

  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }));

  const activeIds = activeWorkspaceId ? (sessionsByWorkspace[activeWorkspaceId] ?? []) : [];
  const visibleIds =
    sessionFilter === ALL_FILTER
      ? activeIds
      : activeIds.filter((id) => groupFor({ sessionGroup }, id) === sessionFilter);

  const storedLayout = activeWorkspaceId ? (gridLayoutMap[activeWorkspaceId] ?? null) : null;
  const layoutIsStale = !storedLayout || !sameIdSet(collectLeafIds(storedLayout), visibleIds);
  const renderLayout = layoutIsStale ? buildBalancedLayout(visibleIds) : storedLayout;

  // The split tree is only rebuilt (losing custom sizes) when which
  // Sessions are visible actually changes — not on every render — so
  // resizing/dragging persists across unrelated re-renders.
  useEffect(() => {
    if (activeWorkspaceId && layoutIsStale) {
      setGridLayout(activeWorkspaceId, renderLayout);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeWorkspaceId, layoutIsStale]);

  const hiddenIds = order.flatMap((workspaceId) => {
    const ids = sessionsByWorkspace[workspaceId] ?? [];
    return workspaceId === activeWorkspaceId ? ids.filter((id) => !visibleIds.includes(id)) : ids;
  });

  function handleDragEnd(event: DragEndEvent) {
    setDraggingId(null);
    setOverId(null);
    if (!activeWorkspaceId || !event.over) return;
    swapGridLeaves(activeWorkspaceId, String(event.active.id), String(event.over.id));
  }

  return (
    <DndContext
      sensors={sensors}
      onDragStart={(event) => setDraggingId(String(event.active.id))}
      onDragOver={(event) => setOverId(event.over ? String(event.over.id) : null)}
      onDragEnd={handleDragEnd}
      onDragCancel={() => {
        setDraggingId(null);
        setOverId(null);
      }}
    >
      <div className={styles.gridRoot}>
        <div className={styles.tree}>
          {renderLayout && activeWorkspaceId && (
            <LayoutView
              node={renderLayout}
              sessions={sessions}
              draggingId={draggingId}
              overId={overId}
              onRequestClose={requestCloseSession}
              onResize={(splitId, fraction) => resizeGridSplit(activeWorkspaceId, splitId, fraction)}
            />
          )}
        </div>
        <div className={styles.hidden}>
          {hiddenIds.map((sessionId) => {
            const session = sessions[sessionId];
            if (!session) return null;
            return (
              <GridCell
                key={sessionId}
                session={session}
                visible={false}
                isDragging={false}
                isDropTarget={false}
                onRequestClose={requestCloseSession}
              />
            );
          })}
        </div>
      </div>
    </DndContext>
  );
}
