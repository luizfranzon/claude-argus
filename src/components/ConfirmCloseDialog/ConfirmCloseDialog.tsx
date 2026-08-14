import * as Dialog from "@radix-ui/react-dialog";

import { useWorkspaceStore } from "../../state/workspaceStore";
import styles from "./ConfirmCloseDialog.module.css";

export function ConfirmCloseDialog() {
  const pendingCloseWorkspaceId = useWorkspaceStore((state) => state.pendingCloseWorkspaceId);
  const workspaces = useWorkspaceStore((state) => state.workspaces);
  const confirmPendingClose = useWorkspaceStore((state) => state.confirmPendingClose);
  const cancelPendingClose = useWorkspaceStore((state) => state.cancelPendingClose);

  const workspace = pendingCloseWorkspaceId ? workspaces[pendingCloseWorkspaceId] : undefined;
  const open = Boolean(pendingCloseWorkspaceId && workspace);

  return (
    <Dialog.Root open={open} onOpenChange={(next) => !next && cancelPendingClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className={styles.overlay} />
        <Dialog.Content className={styles.content}>
          <Dialog.Title className={styles.title}>Terminate process?</Dialog.Title>
          <Dialog.Description className={styles.description}>
            claude is still running in {workspace?.directory}. Closing this workspace will
            terminate it.
          </Dialog.Description>
          <div className={styles.actions}>
            <Dialog.Close asChild>
              <button type="button" className={styles.button} onClick={cancelPendingClose}>
                Cancel
              </button>
            </Dialog.Close>
            <button
              type="button"
              className={`${styles.button} ${styles.destructive}`}
              onClick={() => void confirmPendingClose()}
            >
              Terminate
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
