import { useState, type DragEvent, type MouseEvent } from "react";
import { ChevronDown, ChevronRight, File, Folder } from "lucide-react";

import { basename, joinPath, relativeTo } from "../../lib/paths";
import type { FileEntryDto } from "../../lib/types";
import { useEditorStore } from "../../state/editorStore";
import { useFileTreeStore } from "../../state/fileTreeStore";
import { ContextMenu, type ContextMenuTarget } from "../ContextMenu/ContextMenu";
import styles from "./FileExplorer.module.css";

interface FileTreeNodeProps {
  entry: FileEntryDto;
  depth: number;
  workspaceId: string;
  workspaceRoot: string;
  gitIgnored?: boolean;
}

function quoteIfNeeded(path: string): string {
  return /[\s"'$&|;<>()]/.test(path) ? `"${path}"` : path;
}

export function FileTreeNode({ entry, depth, workspaceId, workspaceRoot, gitIgnored }: FileTreeNodeProps) {
  const expanded = useFileTreeStore((state) => !!state.expanded[entry.path]);
  const dirState = useFileTreeStore((state) => state.dirs[entry.path]);
  const toggleDir = useFileTreeStore((state) => state.toggleDir);
  const createEntry = useFileTreeStore((state) => state.createEntry);
  const renameEntry = useFileTreeStore((state) => state.renameEntry);
  const deleteEntry = useFileTreeStore((state) => state.deleteEntry);
  const movePath = useFileTreeStore((state) => state.movePath);
  const openFile = useEditorStore((state) => state.openFile);

  const [menu, setMenu] = useState<ContextMenuTarget | null>(null);
  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState(entry.name);
  const [dropTarget, setDropTarget] = useState(false);

  const parentDir = entry.path.slice(0, entry.path.length - entry.name.length - 1) || workspaceRoot;

  function handleClick() {
    if (entry.isDir) {
      toggleDir(entry.path);
    } else {
      void openFile(workspaceId, entry.path);
    }
  }

  function handleContextMenu(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    setMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        ...(entry.isDir
          ? [
              { label: "New File", onSelect: () => promptCreate(false) },
              { label: "New Folder", onSelect: () => promptCreate(true) },
            ]
          : []),
        { label: "Rename", onSelect: () => setRenaming(true) },
        {
          label: "Delete",
          danger: true,
          onSelect: () => void deleteEntry(entry.path, parentDir),
        },
      ],
    });
  }

  function promptCreate(isDir: boolean) {
    const name = window.prompt(isDir ? "Folder name" : "File name");
    if (name) void createEntry(entry.path, name, isDir);
  }

  function commitRename() {
    setRenaming(false);
    if (renameValue && renameValue !== entry.name) {
      void renameEntry(entry.path, joinPath(parentDir, renameValue));
    } else {
      setRenameValue(entry.name);
    }
  }

  function handleDragStart(e: DragEvent) {
    const relative = relativeTo(workspaceRoot, entry.path);
    e.dataTransfer.setData("text/plain", quoteIfNeeded(relative));
    e.dataTransfer.setData("application/x-argus-path", entry.path);
    // "copy" for a drop onto the terminal (inserts a path reference), "move"
    // for a drop onto another folder in this tree (relocates the file) — the
    // browser picks the cursor from whichever the target's dragOver allows.
    e.dataTransfer.effectAllowed = "copyMove";
  }

  function handleDragOver(e: DragEvent) {
    if (!entry.isDir || !e.dataTransfer.types.includes("application/x-argus-path")) return;
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = "move";
  }

  function handleDragEnter(e: DragEvent) {
    if (!entry.isDir || !e.dataTransfer.types.includes("application/x-argus-path")) return;
    e.stopPropagation();
    setDropTarget(true);
  }

  function handleDragLeave(e: DragEvent) {
    if (!entry.isDir) return;
    e.stopPropagation();
    setDropTarget(false);
  }

  function handleDrop(e: DragEvent) {
    if (!entry.isDir) return;
    const sourcePath = e.dataTransfer.getData("application/x-argus-path");
    if (!sourcePath) return;
    e.preventDefault();
    e.stopPropagation();
    setDropTarget(false);
    if (sourcePath === entry.path) return;
    const sourceName = basename(sourcePath);
    const destination = joinPath(entry.path, sourceName);
    if (destination === sourcePath) return;
    void movePath(sourcePath, destination, sourcePath.slice(0, sourcePath.length - sourceName.length - 1) || workspaceRoot);
  }

  return (
    <div>
      <div
        className={dropTarget ? `${styles.node} ${styles.nodeDropTarget}` : styles.node}
        style={{ paddingLeft: 8 + depth * 14, opacity: gitIgnored ? 0.5 : 1 }}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
        draggable
        onDragStart={handleDragStart}
        onDragOver={handleDragOver}
        onDragEnter={handleDragEnter}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        title={basename(entry.path)}
      >
        {entry.isDir ? (
          expanded ? (
            <ChevronDown size={13} className={styles.chevron} />
          ) : (
            <ChevronRight size={13} className={styles.chevron} />
          )
        ) : (
          <span className={styles.chevronSpacer} />
        )}
        {entry.isDir ? <Folder size={14} className={styles.icon} /> : <File size={14} className={styles.icon} />}
        {renaming ? (
          <input
            className={styles.renameInput}
            autoFocus
            value={renameValue}
            onClick={(e) => e.stopPropagation()}
            onChange={(e) => setRenameValue(e.target.value)}
            onBlur={commitRename}
            onKeyDown={(e) => {
              if (e.key === "Enter") commitRename();
              if (e.key === "Escape") {
                setRenameValue(entry.name);
                setRenaming(false);
              }
            }}
          />
        ) : (
          <span className={styles.label}>{entry.name}</span>
        )}
      </div>
      {entry.isDir && expanded && (
        <div>
          {dirState?.loading && !dirState.entries.length && (
            <div className={styles.loading} style={{ paddingLeft: 8 + (depth + 1) * 14 }}>
              Loading…
            </div>
          )}
          {dirState?.entries.map((child) => (
            <FileTreeNode
              key={child.path}
              entry={child}
              depth={depth + 1}
              workspaceId={workspaceId}
              workspaceRoot={workspaceRoot}
            />
          ))}
        </div>
      )}
      <ContextMenu target={menu} onClose={() => setMenu(null)} />
    </div>
  );
}
