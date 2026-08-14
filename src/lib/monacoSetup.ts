import * as monaco from "monaco-editor";

// Deliberately no `self.MonacoEnvironment.getWorker`/`getWorkerUrl` here.
// monaco-editor 0.56's ESM build is fully self-bootstrapping: every worker
// manager (json/css/html/typescript, plus the base editor worker service)
// creates its own worker via `new Worker(new URL('xxx.worker.js',
// import.meta.url), {type:'module'})`, relative to monaco's own files — a
// pattern Vite/Rolldown natively detects and bundles correctly, including
// inside a dependency. `MonacoEnvironment.getWorker`/`getWorkerUrl`, if set,
// takes priority over that and replaces it — three attempts at supplying a
// custom one here (the `?worker` import suffix, a hand-built `new
// URL(...,{type:'module'})`, and a classic AMD `importScripts` bootstrap)
// each broke a different way, because they were all fighting monaco's own
// already-correct default instead of just leaving it alone.

// Hardcoded to argus's `dark` theme tokens (src/styles/theme.css) rather than
// read live via CSS vars — Monaco's theme API needs literal hex at
// `defineTheme` time. Keep these in sync if theme.css's dark palette changes.
monaco.editor.defineTheme("argus-dark", {
  base: "vs-dark",
  inherit: true,
  rules: [],
  colors: {
    "editor.background": "#101114",
    "editor.foreground": "#f3f4f6",
    "editor.lineHighlightBackground": "#1a1c1f",
    "editorLineNumber.foreground": "#6b6b75",
    "editorLineNumber.activeForeground": "#c8c8d0",
    "editor.selectionBackground": "#34373e",
    "editorCursor.foreground": "#f3f4f6",
    "editorWidget.background": "#1f2125",
    "editorWidget.border": "#2a2d33",
  },
});

