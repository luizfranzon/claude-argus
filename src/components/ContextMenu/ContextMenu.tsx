import { useEffect, useRef } from "react";

import styles from "./ContextMenu.module.css";

export interface ContextMenuItem {
  label: string;
  onSelect: () => void;
  danger?: boolean;
}

export interface ContextMenuTarget {
  x: number;
  y: number;
  items: ContextMenuItem[];
}

interface ContextMenuProps {
  target: ContextMenuTarget | null;
  onClose: () => void;
}

/** Minimal right-click menu: fixed-position, closes on outside click/Escape/scroll. */
export function ContextMenu({ target, onClose }: ContextMenuProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!target) return;
    function handlePointerDown(event: PointerEvent) {
      if (ref.current && !ref.current.contains(event.target as Node)) {
        onClose();
      }
    }
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    window.addEventListener("pointerdown", handlePointerDown);
    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("scroll", onClose, true);
    return () => {
      window.removeEventListener("pointerdown", handlePointerDown);
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("scroll", onClose, true);
    };
  }, [target, onClose]);

  if (!target) return null;

  return (
    <div ref={ref} className={styles.menu} style={{ left: target.x, top: target.y }}>
      {target.items.map((item) => (
        <button
          key={item.label}
          type="button"
          className={item.danger ? `${styles.item} ${styles.itemDanger}` : styles.item}
          onClick={() => {
            item.onSelect();
            onClose();
          }}
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}
