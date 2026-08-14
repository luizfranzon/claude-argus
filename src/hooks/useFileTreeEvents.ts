import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

import type { FsChangedEvent } from "../lib/types";
import { useEditorStore } from "../state/editorStore";
import { useFileTreeStore } from "../state/fileTreeStore";
import { useGitStore } from "../state/gitStore";
import { useWorkspaceStore } from "../state/workspaceStore";

/**
 * Refreshes every currently-loaded (expanded) directory under a Workspace
 * once the Rust-side file watcher reports a change — covers edits `claude`
 * makes on its own, not just ones argus's own File Explorer CRUD caused.
 */
export function useFileTreeEvents() {
  useEffect(() => {
    const unlisten = listen<FsChangedEvent>("fs-changed", (event) => {
      const workspace = useWorkspaceStore.getState().workspaces[event.payload.workspaceId];
      if (!workspace) return;
      const root = workspace.directory;
      const loadedDirs = Object.keys(useFileTreeStore.getState().dirs);
      for (const dir of loadedDirs) {
        if (dir === root || dir.startsWith(root + "/") || dir.startsWith(root + "\\")) {
          void useFileTreeStore.getState().refreshDir(dir);
        }
      }
      void useEditorStore.getState().checkExternalChanges(event.payload.workspaceId);

      const repos = useGitStore.getState().repositories[event.payload.workspaceId] ?? [];
      for (const repo of repos) {
        void useGitStore.getState().refreshRepo(repo.path);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);
}
