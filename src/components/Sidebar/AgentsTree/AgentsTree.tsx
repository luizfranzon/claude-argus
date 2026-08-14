import { useState, type DragEvent, type MouseEvent } from "react";
import { Bot, FolderPlus, MoreHorizontal, Plus, Users } from "lucide-react";

import {
  ALL_FILTER,
  groupFor,
  UNGROUPED_FILTER,
  useFeatureGroupStore,
} from "../../../state/featureGroupStore";
import { useSessionStore } from "../../../state/sessionStore";
import { EMPTY_ARRAY } from "../../../lib/emptyArray";
import { ContextMenu, type ContextMenuTarget } from "../../ContextMenu/ContextMenu";
import fileStyles from "../../FileExplorer/FileExplorer.module.css";
import styles from "./AgentsTree.module.css";

const SESSION_MIME = "application/x-argus-session";

interface AgentsTreeProps {
  workspaceId: string;
}

function initials(label: string): string {
  return label.trim().slice(0, 2).toLowerCase();
}

/**
 * The Agents sidebar tab: a card per "All", "Ungrouped", and each Feature
 * Group, listing that Workspace's Sessions. Clicking a card header both
 * expands it and selects it as the Session Filter that drives the Grid
 * (single action — see CONTEXT.md's "Session Filter"). Dragging a Session
 * row onto a group card reassigns it.
 */
export function AgentsTree({ workspaceId }: AgentsTreeProps) {
  const sessionsByWorkspace = useSessionStore((state) => state.sessionsByWorkspace);
  const sessionGroup = useFeatureGroupStore((state) => state.sessionGroup);
  const groupIds = useFeatureGroupStore((state) => state.groupsByWorkspace[workspaceId] ?? EMPTY_ARRAY);
  const groups = useFeatureGroupStore((state) => state.groups);
  const deleteFeatureGroup = useFeatureGroupStore((state) => state.deleteFeatureGroup);
  const renameFeatureGroup = useFeatureGroupStore((state) => state.renameFeatureGroup);
  const openCreateFeatureGroup = useFeatureGroupStore((state) => state.openCreateFeatureGroup);
  const createSession = useSessionStore((state) => state.createSession);
  const assignSessionToGroup = useFeatureGroupStore((state) => state.assignSessionToGroup);

  const allIds = sessionsByWorkspace[workspaceId] ?? [];
  const ungroupedIds = allIds.filter((id) => groupFor({ sessionGroup }, id) === UNGROUPED_FILTER);

  async function createSessionIn(groupId?: string) {
    const session = await createSession(workspaceId);
    if (groupId) assignSessionToGroup(session.id, groupId);
  }

  return (
    <div className={fileStyles.explorer}>
      <div className={fileStyles.toolbar}>
        <button
          type="button"
          className={fileStyles.toolbarButton}
          title="New session"
          onClick={() => void createSessionIn()}
        >
          <Plus size={14} />
        </button>
        <button
          type="button"
          className={fileStyles.toolbarButton}
          title="New feature group"
          onClick={() => openCreateFeatureGroup(workspaceId)}
        >
          <FolderPlus size={14} />
        </button>
      </div>
      <div className={styles.cards}>
        <GroupCard id={ALL_FILTER} label="All" sessionIds={allIds} workspaceId={workspaceId} />
        <GroupCard
          id={UNGROUPED_FILTER}
          label="Ungrouped"
          sessionIds={ungroupedIds}
          workspaceId={workspaceId}
          droppable
          onAddSession={() => void createSessionIn()}
        />
        {groupIds.map((id) => {
          const group = groups[id];
          if (!group) return null;
          const sessionIds = allIds.filter((sid) => sessionGroup[sid] === id);
          return (
            <GroupCard
              key={id}
              id={id}
              label={group.name}
              color={group.color}
              sessionIds={sessionIds}
              workspaceId={workspaceId}
              droppable
              deletable
              onDelete={() => deleteFeatureGroup(id)}
              onRename={(name) => renameFeatureGroup(id, name)}
              onAddSession={() => void createSessionIn(id)}
            />
          );
        })}
      </div>
    </div>
  );
}

interface GroupCardProps {
  id: string;
  label: string;
  color?: string;
  sessionIds: string[];
  workspaceId: string;
  droppable?: boolean;
  deletable?: boolean;
  onDelete?: () => void;
  onRename?: (name: string) => void;
  onAddSession?: () => void;
}

function GroupCard({
  id,
  label,
  color,
  sessionIds,
  workspaceId,
  droppable,
  deletable,
  onDelete,
  onRename,
  onAddSession,
}: GroupCardProps) {
  const expanded = useFeatureGroupStore((state) => !!state.expanded[id]);
  const toggleGroupExpanded = useFeatureGroupStore((state) => state.toggleGroupExpanded);
  const activeFilter = useFeatureGroupStore((state) => state.sessionFilter[workspaceId] ?? ALL_FILTER);
  const setSessionFilter = useFeatureGroupStore((state) => state.setSessionFilter);
  const assignSessionToGroup = useFeatureGroupStore((state) => state.assignSessionToGroup);

  const [dropTarget, setDropTarget] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState(label);
  const [menu, setMenu] = useState<ContextMenuTarget | null>(null);

  const isActiveFilter = activeFilter === id;

  function handleClick() {
    toggleGroupExpanded(id);
    setSessionFilter(workspaceId, id);
  }

  function openMenu(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    setMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: "Rename", onSelect: () => setRenaming(true) },
        { label: "Delete group", danger: true, onSelect: () => onDelete?.() },
      ],
    });
  }

  function commitRename() {
    setRenaming(false);
    const trimmed = renameValue.trim();
    if (trimmed && trimmed !== label) {
      onRename?.(trimmed);
    } else {
      setRenameValue(label);
    }
  }

  function handleDragOver(e: DragEvent) {
    if (!droppable || !e.dataTransfer.types.includes(SESSION_MIME)) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
  }

  function handleDrop(e: DragEvent) {
    if (!droppable) return;
    const sessionId = e.dataTransfer.getData(SESSION_MIME);
    if (!sessionId) return;
    e.preventDefault();
    setDropTarget(false);
    assignSessionToGroup(sessionId, id);
  }

  return (
    <div className={[styles.card, dropTarget && styles.cardDropTarget].filter(Boolean).join(" ")}>
      <div
        className={isActiveFilter ? `${styles.cardHeader} ${styles.cardHeaderActive}` : styles.cardHeader}
        onClick={handleClick}
        onContextMenu={deletable ? openMenu : undefined}
        onDragOver={handleDragOver}
        onDragEnter={() => droppable && setDropTarget(true)}
        onDragLeave={() => setDropTarget(false)}
        onDrop={handleDrop}
      >
        {color ? (
          <span className={styles.avatar} style={{ background: `var(${color})` }}>
            {initials(label)}
          </span>
        ) : (
          <span className={styles.avatarNeutral}>
            <Users size={12} />
          </span>
        )}
        {renaming ? (
          <input
            className={fileStyles.renameInput}
            autoFocus
            value={renameValue}
            onClick={(e) => e.stopPropagation()}
            onChange={(e) => setRenameValue(e.target.value)}
            onBlur={commitRename}
            onKeyDown={(e) => {
              if (e.key === "Enter") commitRename();
              if (e.key === "Escape") {
                setRenameValue(label);
                setRenaming(false);
              }
            }}
          />
        ) : (
          <span className={styles.cardName}>{label}</span>
        )}
        {!expanded && <span className={styles.count}>{sessionIds.length}</span>}
        {onAddSession && (
          <button
            type="button"
            className={styles.cardAction}
            title="New session in this group"
            onClick={(e) => {
              e.stopPropagation();
              onAddSession();
            }}
          >
            <Plus size={13} />
          </button>
        )}
        {deletable && (
          <button type="button" className={styles.cardAction} title="More" onClick={openMenu}>
            <MoreHorizontal size={13} />
          </button>
        )}
      </div>
      {expanded && (
        <div className={styles.sessionList}>
          {sessionIds.length === 0 && <div className={styles.emptyHint}>No sessions</div>}
          {sessionIds.map((sessionId) => (
            <SessionRow key={sessionId} sessionId={sessionId} />
          ))}
        </div>
      )}
      <ContextMenu target={menu} onClose={() => setMenu(null)} />
    </div>
  );
}

function SessionRow({ sessionId }: { sessionId: string }) {
  const session = useSessionStore((state) => state.sessions[sessionId]);
  const runtimeStatus = useSessionStore((state) => state.runtimeStatus[sessionId]);
  const requestCloseSession = useSessionStore((state) => state.requestCloseSession);
  const renameSession = useSessionStore((state) => state.renameSession);
  const groups = useFeatureGroupStore((state) => state.groups);
  const groupsByWorkspace = useFeatureGroupStore((state) => state.groupsByWorkspace);
  const assignSessionToGroup = useFeatureGroupStore((state) => state.assignSessionToGroup);

  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState(session?.name ?? "");
  const [menu, setMenu] = useState<ContextMenuTarget | null>(null);

  if (!session) return null;

  function handleDragStart(e: DragEvent) {
    e.dataTransfer.setData(SESSION_MIME, session!.id);
    e.dataTransfer.effectAllowed = "move";
  }

  function commitRename() {
    setRenaming(false);
    const trimmed = renameValue.trim();
    if (trimmed && trimmed !== session!.name) {
      void renameSession(session!.id, trimmed);
    } else {
      setRenameValue(session!.name);
    }
  }

  const workspaceGroups = (groupsByWorkspace[session.workspaceId] ?? [])
    .map((id) => groups[id])
    .filter((g): g is NonNullable<typeof g> => Boolean(g));

  return (
    <>
      <div
        className={styles.sessionRow}
        draggable
        onDragStart={handleDragStart}
        onContextMenu={(e) => {
          e.preventDefault();
          e.stopPropagation();
          setMenu({
            x: e.clientX,
            y: e.clientY,
            items: [
              { label: "Rename", onSelect: () => setRenaming(true) },
              { label: "Move to Ungrouped", onSelect: () => assignSessionToGroup(session.id, UNGROUPED_FILTER) },
              ...workspaceGroups.map((g) => ({
                label: `Move to ${g.name}`,
                onSelect: () => assignSessionToGroup(session.id, g.id),
              })),
              { label: "Close session", danger: true, onSelect: () => void requestCloseSession(session.id) },
            ],
          });
        }}
      >
        <span className={styles.sessionIcon}>
          <Bot size={12} />
        </span>
        {renaming ? (
          <input
            className={fileStyles.renameInput}
            autoFocus
            value={renameValue}
            onClick={(e) => e.stopPropagation()}
            onChange={(e) => setRenameValue(e.target.value)}
            onBlur={commitRename}
            onKeyDown={(e) => {
              if (e.key === "Enter") commitRename();
              if (e.key === "Escape") {
                setRenameValue(session.name);
                setRenaming(false);
              }
            }}
          />
        ) : (
          <span className={styles.sessionName} onDoubleClick={() => setRenaming(true)}>
            {session.name}
          </span>
        )}
        <span
          className={runtimeStatus === "thinking" ? `${styles.statusDot} ${styles.statusThinking}` : styles.statusDot}
          title={runtimeStatus === "thinking" ? "Thinking" : runtimeStatus === "idle" ? "Idle" : "Status unknown"}
        />
      </div>
      <ContextMenu target={menu} onClose={() => setMenu(null)} />
    </>
  );
}
