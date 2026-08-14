import { FolderOpen } from "lucide-react";

import { useWorkspaceStore } from "../../state/workspaceStore";
import styles from "../WelcomeScreen/WelcomeScreen.module.css";

/**
 * Shown only before the first workspace exists when argus was launched from
 * a GUI icon (no CLI cwd to default to) — see `--gui-launch` handling in
 * src-tauri. Reuses WelcomeScreen's styling since the shape is identical,
 * only the copy differs.
 */
export function DirectoryPickerScreen() {
  const createWorkspaceViaPicker = useWorkspaceStore((state) => state.createWorkspaceViaPicker);
  const startupPathStatus = useWorkspaceStore((state) => state.startupPathStatus);

  return (
    <div className={styles.container}>
      <FolderOpen size={40} className={styles.icon} />
      <div className={styles.title}>Choose a directory to get started</div>
      <button
        type="button"
        className={styles.button}
        disabled={startupPathStatus !== "ready"}
        onClick={() => void createWorkspaceViaPicker()}
      >
        <FolderOpen size={16} />
        Choose directory
      </button>
    </div>
  );
}
