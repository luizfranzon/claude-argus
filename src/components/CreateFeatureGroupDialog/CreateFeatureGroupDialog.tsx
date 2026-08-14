import { useEffect, useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";

import { useFeatureGroupStore } from "../../state/featureGroupStore";
import styles from "./CreateFeatureGroupDialog.module.css";

const COLORS = [
  "--group-color-1",
  "--group-color-2",
  "--group-color-3",
  "--group-color-4",
  "--group-color-5",
  "--group-color-6",
  "--group-color-7",
  "--group-color-8",
];

export function CreateFeatureGroupDialog() {
  const pendingWorkspaceId = useFeatureGroupStore((state) => state.pendingCreateFeatureGroupWorkspaceId);
  const cancelCreateFeatureGroup = useFeatureGroupStore((state) => state.cancelCreateFeatureGroup);
  const createFeatureGroup = useFeatureGroupStore((state) => state.createFeatureGroup);

  const [name, setName] = useState("");
  const [color, setColor] = useState(COLORS[0]);

  const open = Boolean(pendingWorkspaceId);

  useEffect(() => {
    if (open) {
      setName("");
      setColor(COLORS[0]);
    }
  }, [open]);

  function handleCreate() {
    const trimmed = name.trim();
    if (!trimmed) return;
    createFeatureGroup(trimmed, color);
  }

  return (
    <Dialog.Root open={open} onOpenChange={(next) => !next && cancelCreateFeatureGroup()}>
      <Dialog.Portal>
        <Dialog.Overlay className={styles.overlay} />
        <Dialog.Content className={styles.content}>
          <Dialog.Title className={styles.title}>New feature group</Dialog.Title>
          <Dialog.Description className={styles.description}>
            Group sessions working on the same feature together.
          </Dialog.Description>

          <label className={styles.label} htmlFor="feature-group-name">
            Name
          </label>
          <input
            id="feature-group-name"
            className={styles.input}
            autoFocus
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleCreate()}
            placeholder="e.g. Checkout redesign"
          />

          <span className={styles.label}>Color</span>
          <div className={styles.swatches}>
            {COLORS.map((token) => (
              <button
                key={token}
                type="button"
                aria-pressed={color === token}
                className={color === token ? `${styles.swatch} ${styles.swatchSelected}` : styles.swatch}
                style={{ background: `var(${token})` }}
                onClick={() => setColor(token)}
                title={token}
              />
            ))}
          </div>

          <div className={styles.actions}>
            <Dialog.Close asChild>
              <button type="button" className={styles.button} onClick={cancelCreateFeatureGroup}>
                Cancel
              </button>
            </Dialog.Close>
            <button
              type="button"
              className={`${styles.button} ${styles.primary}`}
              disabled={!name.trim()}
              onClick={handleCreate}
            >
              Create
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
