import { useEffect, useRef } from "react";
import * as monaco from "monaco-editor";
import { X } from "lucide-react";

import { EMPTY_ARRAY } from "../../lib/emptyArray";
import { basename } from "../../lib/paths";
import { getOrLoadModel, isDirty } from "../../lib/monacoModels";
import "../../lib/monacoSetup";
import { useEditorStore } from "../../state/editorStore";
import styles from "./EditorPane.module.css";

interface EditorPaneProps {
  workspaceId: string;
}

function readToken(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

export function EditorPane({ workspaceId }: EditorPaneProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const contentSubRef = useRef<monaco.IDisposable | null>(null);

  const tabs = useEditorStore((state) => state.openFiles[workspaceId] ?? EMPTY_ARRAY);
  const activePath = useEditorStore((state) => state.activeFile[workspaceId] ?? null);
  const closeFile = useEditorStore((state) => state.closeFile);
  const setActiveFile = useEditorStore((state) => state.setActiveFile);
  const setDirty = useEditorStore((state) => state.setDirty);
  const resolveConflict = useEditorStore((state) => state.resolveConflict);

  const activeTab = tabs.find((tab) => tab.path === activePath);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const editor = monaco.editor.create(container, {
      theme: "argus-dark",
      automaticLayout: true,
      fontFamily: readToken("--font-mono") || "monospace",
      fontSize: 13,
      minimap: { enabled: false },
      scrollBeyondLastLine: false,
    });
    editorRef.current = editor;

    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
      const path = useEditorStore.getState().activeFile[workspaceId];
      if (path) void useEditorStore.getState().saveFile(workspaceId, path);
    });

    return () => {
      contentSubRef.current?.dispose();
      editor.dispose();
      editorRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [workspaceId]);

  useEffect(() => {
    const editor = editorRef.current;
    if (!editor || !activePath) return;
    let cancelled = false;

    void getOrLoadModel(activePath).then((model) => {
      if (cancelled) return;
      editor.setModel(model);
      contentSubRef.current?.dispose();
      contentSubRef.current = model.onDidChangeContent(() => {
        setDirty(workspaceId, activePath, isDirty(activePath));
      });
    });

    return () => {
      cancelled = true;
    };
  }, [activePath, workspaceId, setDirty]);

  if (tabs.length === 0) return null;

  return (
    <div className={styles.pane}>
      <div className={styles.tabStrip}>
        {tabs.map((tab) => (
          <div
            key={tab.path}
            className={tab.path === activePath ? `${styles.tab} ${styles.tabActive}` : styles.tab}
            onClick={() => setActiveFile(workspaceId, tab.path)}
            title={tab.path}
          >
            <span className={styles.tabLabel}>{basename(tab.path)}</span>
            {tab.dirty && <span className={styles.dirtyDot} />}
            <button
              type="button"
              className={styles.closeButton}
              onClick={(e) => {
                e.stopPropagation();
                closeFile(workspaceId, tab.path);
              }}
            >
              <X size={12} />
            </button>
          </div>
        ))}
      </div>
      {activeTab?.conflict && (
        <div className={styles.conflictBanner}>
          <span>This file changed on disk while you had unsaved edits.</span>
          <button type="button" onClick={() => void resolveConflict(workspaceId, activeTab.path, true)}>
            Keep mine
          </button>
          <button type="button" onClick={() => void resolveConflict(workspaceId, activeTab.path, false)}>
            Reload from disk
          </button>
        </div>
      )}
      <div ref={containerRef} className={styles.editorContainer} />
    </div>
  );
}
