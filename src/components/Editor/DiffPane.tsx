import { useEffect, useRef } from "react";
import * as monaco from "monaco-editor";
import { X } from "lucide-react";

import { languageForPath } from "../../lib/languageForPath";
import "../../lib/monacoSetup";
import * as tauriApi from "../../lib/tauri";
import { useGitStore } from "../../state/gitStore";
import styles from "./DiffPane.module.css";

export function DiffPane() {
  const target = useGitStore((state) => state.activeDiff);
  const closeDiff = useGitStore((state) => state.closeDiff);
  const containerRef = useRef<HTMLDivElement>(null);
  const diffEditorRef = useRef<monaco.editor.IStandaloneDiffEditor | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const diffEditor = monaco.editor.createDiffEditor(container, {
      theme: "argus-dark",
      automaticLayout: true,
      readOnly: true,
      renderSideBySide: true,
      minimap: { enabled: false },
    });
    diffEditorRef.current = diffEditor;
    return () => {
      diffEditor.dispose();
      diffEditorRef.current = null;
    };
  }, []);

  useEffect(() => {
    if (!target || !diffEditorRef.current) return;
    let cancelled = false;
    let original: monaco.editor.ITextModel | null = null;
    let modified: monaco.editor.ITextModel | null = null;

    void tauriApi.gitDiff(target.repoPath, target.file, target.staged).then((content) => {
      if (cancelled || !diffEditorRef.current) return;
      const language = languageForPath(target.file);
      original = monaco.editor.createModel(content.old, language);
      modified = monaco.editor.createModel(content.new, language);
      diffEditorRef.current.setModel({ original, modified });
    });

    return () => {
      cancelled = true;
      original?.dispose();
      modified?.dispose();
    };
  }, [target]);

  if (!target) return null;

  return (
    <div className={styles.pane}>
      <div className={styles.header}>
        <span className={styles.fileName}>
          {target.file} {target.staged ? "(staged)" : "(working tree)"}
        </span>
        <button type="button" className={styles.closeButton} onClick={closeDiff}>
          <X size={14} />
        </button>
      </div>
      <div ref={containerRef} className={styles.editorContainer} />
    </div>
  );
}
