import * as Dialog from "@radix-ui/react-dialog";

import { useSessionStore } from "../../state/sessionStore";
import styles from "./ConfirmCloseSessionDialog.module.css";

export function ConfirmCloseSessionDialog() {
  const pendingCloseSessionId = useSessionStore((state) => state.pendingCloseSessionId);
  const sessions = useSessionStore((state) => state.sessions);
  const confirmPendingClose = useSessionStore((state) => state.confirmPendingClose);
  const cancelPendingClose = useSessionStore((state) => state.cancelPendingClose);

  const session = pendingCloseSessionId ? sessions[pendingCloseSessionId] : undefined;
  const open = Boolean(pendingCloseSessionId && session);

  return (
    <Dialog.Root open={open} onOpenChange={(next) => !next && cancelPendingClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className={styles.overlay} />
        <Dialog.Content className={styles.content}>
          <Dialog.Title className={styles.title}>Terminate process?</Dialog.Title>
          <Dialog.Description className={styles.description}>
            claude is still running in {session?.name}. Closing this session will terminate it.
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
