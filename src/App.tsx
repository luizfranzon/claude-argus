import { useEffect, useRef, useState } from "react";

import { ConfirmCloseDialog } from "./components/ConfirmCloseDialog/ConfirmCloseDialog";
import { DirectoryPickerScreen } from "./components/DirectoryPickerScreen/DirectoryPickerScreen";
import { DiffPane } from "./components/Editor/DiffPane";
import { EditorPane } from "./components/Editor/EditorPane";
import { Sidebar } from "./components/Sidebar/Sidebar";
import { SplitView } from "./components/SplitView/SplitView";
import { StartupScreen } from "./components/StartupScreen/StartupScreen";
import { TerminalView } from "./components/TerminalView/TerminalView";
import { TopBar } from "./components/TopBar/TopBar";
import { WelcomeScreen } from "./components/WelcomeScreen/WelcomeScreen";
import { useFileTreeEvents } from "./hooks/useFileTreeEvents";
import { useWorkspaceEvents } from "./hooks/useWorkspaceEvents";
import * as tauriApi from "./lib/tauri";
import { useEditorStore } from "./state/editorStore";
import { useGitStore } from "./state/gitStore";
import { useWorkspaceStore } from "./state/workspaceStore";
import styles from "./App.module.css";

export default function App() {
  useWorkspaceEvents();
  useFileTreeEvents();

  const [initialDirectory, setInitialDirectory] = useState<string | null | undefined>(undefined);
  const autoCreateAttempted = useRef(false);
  // Distinguishes "no workspace yet" (show the bootstrap screen) from
  // "had workspaces, user closed the last one" (show WelcomeScreen instead).
  const hasOpenedFirstWorkspace = useRef(false);

  const startupPathStatus = useWorkspaceStore((state) => state.startupPathStatus);
  const startupPathError = useWorkspaceStore((state) => state.startupPathError);
  const order = useWorkspaceStore((state) => state.order);
  const activeWorkspaceId = useWorkspaceStore((state) => state.activeWorkspaceId);
  const activeHasOpenFiles = useEditorStore(
    (state) => !!activeWorkspaceId && (state.openFiles[activeWorkspaceId]?.length ?? 0) > 0,
  );
  const activeDiff = useGitStore((state) => state.activeDiff);
  const createWorkspaceWithDirectory = useWorkspaceStore(
    (state) => state.createWorkspaceWithDirectory,
  );

  useEffect(() => {
    void tauriApi.getInitialDirectory().then(setInitialDirectory);
  }, []);

  useEffect(() => {
    // Call the command directly (request/response) rather than only relying
    // on the `startup-path-resolved` event — a fire-and-forget event emitted
    // from Rust's `.setup()` hook can land before the frontend's listener is
    // registered, since the event has no replay buffer for late subscribers.
    tauriApi
      .resolveStartupPath()
      .then(() => useWorkspaceStore.getState().markStartupPathResolved(true))
      .catch((error) =>
        useWorkspaceStore.getState().markStartupPathResolved(false, String(error)),
      );
  }, []);

  useEffect(() => {
    if (order.length > 0) {
      hasOpenedFirstWorkspace.current = true;
    }
  }, [order.length]);

  // CLI launch: once PATH resolution is ready and the initial directory is
  // known, open the first workspace automatically. GUI launch (initialDirectory
  // is null) skips this — DirectoryPickerScreen drives creation instead.
  useEffect(() => {
    if (
      startupPathStatus === "ready" &&
      initialDirectory &&
      order.length === 0 &&
      !autoCreateAttempted.current
    ) {
      autoCreateAttempted.current = true;
      void createWorkspaceWithDirectory(initialDirectory);
    }
  }, [startupPathStatus, initialDirectory, order.length, createWorkspaceWithDirectory]);

  if (initialDirectory === undefined || startupPathStatus === "pending") {
    return <StartupScreen />;
  }

  if (startupPathStatus === "error") {
    return <StartupScreen error={startupPathError} />;
  }

  if (order.length === 0 && !hasOpenedFirstWorkspace.current) {
    return initialDirectory === null ? <DirectoryPickerScreen /> : <StartupScreen />;
  }

  return (
    <div className={styles.app}>
      <TopBar />
      <div className={styles.body}>
        {activeWorkspaceId && <Sidebar workspaceId={activeWorkspaceId} />}
        <div className={styles.main}>
          <SplitView
            left={
              <div className={styles.terminals}>
                {order.map((id) => (
                  <TerminalView key={id} workspaceId={id} active={id === activeWorkspaceId} />
                ))}
                {order.length === 0 && <WelcomeScreen />}
              </div>
            }
            right={
              activeDiff ? (
                <DiffPane />
              ) : activeWorkspaceId && activeHasOpenFiles ? (
                <EditorPane workspaceId={activeWorkspaceId} />
              ) : null
            }
          />
        </div>
      </div>
      <ConfirmCloseDialog />
    </div>
  );
}
