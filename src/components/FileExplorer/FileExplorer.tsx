import { useEffect, useState, type DragEvent } from "react";
import { FilePlus, FolderPlus } from "lucide-react";

import { basename, dirname, joinPath } from "../../lib/paths";
import { useFileTreeStore } from "../../state/fileTreeStore";
import { useWorkspaceStore } from "../../state/workspaceStore";
import { ContextMenu, type ContextMenuTarget } from "../ContextMenu/ContextMenu";
import { FileTreeNode } from "./FileTreeNode";
import styles from "./FileExplorer.module.css";

interface FileExplorerProps {
  workspaceId: string;
}

export function FileExplorer({ workspaceId }: FileExplorerProps) {
  const workspace = useWorkspaceStore((state) => state.workspaces[workspaceId]);
  const root = workspace?.directory;
  const rootState = useFileTreeStore((state) => (root ? state.dirs[root] : undefined));
  const loadDir = useFileTreeStore((state) => state.loadDir);
  const createEntry = useFileTreeStore((state) => state.createEntry);
  const movePath = useFileTreeStore((state) => state.movePath);
  const [menu, setMenu] = useState<ContextMenuTarget | null>(null);
  const [rootDropTarget, setRootDropTarget] = useState(false);

  useEffect(() => {
    if (root) void loadDir(root);
  }, [root, loadDir]);

  if (!root) return null;

  function promptCreate(isDir: boolean) {
    const name = window.prompt(isDir ? "Folder name" : "File name");
    if (name && root) void createEntry(root, name, isDir);
  }

  return (
    <div
      className={styles.explorer}
      onContextMenu={(e) => {
        e.preventDefault();
        setMenu({
          x: e.clientX,
          y: e.clientY,
          items: [
            { label: "New File", onSelect: () => promptCreate(false) },
            { label: "New Folder", onSelect: () => promptCreate(true) },
          ],
        });
      }}
    >
      <div className={styles.toolbar}>
        <button type="button" className={styles.toolbarButton} title="New File" onClick={() => promptCreate(false)}>
          <FilePlus size={14} />
        </button>
        <button
          type="button"
          className={styles.toolbarButton}
          title="New Folder"
          onClick={() => promptCreate(true)}
        >
          <FolderPlus size={14} />
        </button>
      </div>
      <div
        className={rootDropTarget ? `${styles.tree} ${styles.treeDropTarget}` : styles.tree}
        onDragOver={(e) => {
          if (!e.dataTransfer.types.includes("application/x-argus-path")) return;
          e.preventDefault();
          e.dataTransfer.dropEffect = "move";
        }}
        onDragEnter={(e) => {
          if (!e.dataTransfer.types.includes("application/x-argus-path")) return;
          setRootDropTarget(true);
        }}
        onDragLeave={() => setRootDropTarget(false)}
        onDrop={(e: DragEvent) => {
          const sourcePath = e.dataTransfer.getData("application/x-argus-path");
          if (!sourcePath || !root) return;
          e.preventDefault();
          setRootDropTarget(false);
          const sourceName = basename(sourcePath);
          const destination = joinPath(root, sourceName);
          if (destination === sourcePath) return;
          void movePath(sourcePath, destination, dirname(sourcePath));
        }}
      >
        {rootState?.loading && !rootState.entries.length && <div className={styles.loading}>Loading…</div>}
        {rootState?.entries.map((entry) => (
          <FileTreeNode key={entry.path} entry={entry} depth={0} workspaceId={workspaceId} workspaceRoot={root} />
        ))}
      </div>
      <ContextMenu target={menu} onClose={() => setMenu(null)} />
    </div>
  );
}
