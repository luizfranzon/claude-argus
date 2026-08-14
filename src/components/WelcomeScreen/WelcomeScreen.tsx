import { FolderOpen, TerminalSquare } from "lucide-react";

import { useWorkspaceStore } from "../../state/workspaceStore";
import styles from "./WelcomeScreen.module.css";

/** Shown whenever there are no open workspaces (e.g. after closing the last tab). */
export function WelcomeScreen() {
  const createWorkspaceViaPicker = useWorkspaceStore((state) => state.createWorkspaceViaPicker);
  const startupPathStatus = useWorkspaceStore((state) => state.startupPathStatus);

  return (
    <div className={styles.container}>
      <TerminalSquare size={40} className={styles.icon} />
      <div className={styles.title}>No workspaces open</div>
      <button
        type="button"
        className={styles.button}
        disabled={startupPathStatus !== "ready"}
        onClick={() => void createWorkspaceViaPicker()}
      >
        <FolderOpen size={16} />
        Open a directory
      </button>
    </div>
  );
}
