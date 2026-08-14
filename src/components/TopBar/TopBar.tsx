import { Copy, Plus, X } from "lucide-react";

import { useWorkspaceStore } from "../../state/workspaceStore";
import styles from "./TopBar.module.css";

function basename(path: string): string {
  const normalized = path.replace(/\\/g, "/").replace(/\/+$/, "");
  const parts = normalized.split("/");
  return parts[parts.length - 1] || path;
}

export function TopBar() {
  const order = useWorkspaceStore((state) => state.order);
  const workspaces = useWorkspaceStore((state) => state.workspaces);
  const activeWorkspaceId = useWorkspaceStore((state) => state.activeWorkspaceId);
  const setActiveWorkspace = useWorkspaceStore((state) => state.setActiveWorkspace);
  const requestCloseWorkspace = useWorkspaceStore((state) => state.requestCloseWorkspace);
  const duplicateWorkspace = useWorkspaceStore((state) => state.duplicateWorkspace);
  const createWorkspaceViaPicker = useWorkspaceStore((state) => state.createWorkspaceViaPicker);
  const startupPathStatus = useWorkspaceStore((state) => state.startupPathStatus);

  const canCreate = startupPathStatus === "ready";

  return (
    <div className={styles.bar}>
      <div className={styles.tabStrip}>
        {order.map((id) => {
          const workspace = workspaces[id];
          if (!workspace) return null;
          const active = id === activeWorkspaceId;
          return (
            <div
              key={id}
              className={active ? `${styles.tab} ${styles.tabActive}` : styles.tab}
              onClick={() => setActiveWorkspace(id)}
              title={workspace.directory}
            >
              <span className={styles.tabLabel}>{basename(workspace.directory)}</span>
              <button
                type="button"
                className={styles.iconButton}
                title="Duplicate workspace"
                onClick={(e) => {
                  e.stopPropagation();
                  void duplicateWorkspace(id);
                }}
              >
                <Copy size={13} />
              </button>
              <button
                type="button"
                className={styles.iconButton}
                title="Close workspace"
                onClick={(e) => {
                  e.stopPropagation();
                  void requestCloseWorkspace(id);
                }}
              >
                <X size={13} />
              </button>
            </div>
          );
        })}
      </div>
      <button
        type="button"
        className={styles.newTabButton}
        title="New workspace"
        disabled={!canCreate}
        onClick={() => void createWorkspaceViaPicker()}
      >
        <Plus size={16} />
      </button>
    </div>
  );
}
