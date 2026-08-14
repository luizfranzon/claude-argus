import { useRef, useState, type PointerEvent } from "react";
import { FolderTree, GitBranch, PanelLeftClose, PanelLeftOpen } from "lucide-react";

import { useWorkspaceStore } from "../../state/workspaceStore";
import { FileExplorer } from "../FileExplorer/FileExplorer";
import { GitPanel } from "../GitPanel/GitPanel";
import styles from "./Sidebar.module.css";

interface SidebarProps {
  workspaceId: string;
}

const MIN_WIDTH = 180;
const MAX_WIDTH = 480;
const DEFAULT_WIDTH = 260;

export function Sidebar({ workspaceId }: SidebarProps) {
  const sidebar = useWorkspaceStore((state) => state.sidebar[workspaceId]);
  const setSidebarTab = useWorkspaceStore((state) => state.setSidebarTab);
  const toggleSidebarCollapsed = useWorkspaceStore((state) => state.toggleSidebarCollapsed);

  // One shared width across workspaces — a layout preference, not per-Workspace
  // domain state, same as VS Code's activity bar width.
  const [width, setWidth] = useState(DEFAULT_WIDTH);
  const draggingRef = useRef(false);
  const startRef = useRef({ x: 0, width: DEFAULT_WIDTH });

  const activeTab = sidebar?.activeTab ?? "explorer";
  const collapsed = sidebar?.collapsed ?? false;

  function handleResizePointerDown(e: PointerEvent<HTMLDivElement>) {
    draggingRef.current = true;
    startRef.current = { x: e.clientX, width };
    e.currentTarget.setPointerCapture(e.pointerId);
  }

  function handleResizePointerMove(e: PointerEvent<HTMLDivElement>) {
    if (!draggingRef.current) return;
    const next = startRef.current.width + (e.clientX - startRef.current.x);
    setWidth(Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, next)));
  }

  function handleResizePointerUp() {
    draggingRef.current = false;
  }

  if (collapsed) {
    return (
      <div className={styles.rail}>
        <button
          type="button"
          className={styles.railButton}
          title="Expand sidebar"
          onClick={() => toggleSidebarCollapsed(workspaceId)}
        >
          <PanelLeftOpen size={16} />
        </button>
      </div>
    );
  }

  return (
    <div className={styles.sidebar} style={{ width }}>
      <div className={styles.tabStrip}>
        <button
          type="button"
          className={activeTab === "explorer" ? `${styles.tab} ${styles.tabActive}` : styles.tab}
          title="Explorer"
          onClick={() => setSidebarTab(workspaceId, "explorer")}
        >
          <FolderTree size={15} />
        </button>
        <button
          type="button"
          className={activeTab === "git" ? `${styles.tab} ${styles.tabActive}` : styles.tab}
          title="Source Control"
          onClick={() => setSidebarTab(workspaceId, "git")}
        >
          <GitBranch size={15} />
        </button>
        <div className={styles.spacer} />
        <button
          type="button"
          className={styles.collapseButton}
          title="Collapse sidebar"
          onClick={() => toggleSidebarCollapsed(workspaceId)}
        >
          <PanelLeftClose size={15} />
        </button>
      </div>
      <div className={styles.content}>
        {activeTab === "explorer" ? (
          <FileExplorer workspaceId={workspaceId} />
        ) : (
          <GitPanel workspaceId={workspaceId} />
        )}
      </div>
      <div
        className={styles.resizeHandle}
        onPointerDown={handleResizePointerDown}
        onPointerMove={handleResizePointerMove}
        onPointerUp={handleResizePointerUp}
      />
    </div>
  );
}
