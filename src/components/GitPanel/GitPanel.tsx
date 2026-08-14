import { useEffect } from "react";

import { EMPTY_ARRAY } from "../../lib/emptyArray";
import { useGitStore } from "../../state/gitStore";
import { useWorkspaceStore } from "../../state/workspaceStore";
import { RepositorySection } from "./RepositorySection";
import styles from "./GitPanel.module.css";

interface GitPanelProps {
  workspaceId: string;
}

export function GitPanel({ workspaceId }: GitPanelProps) {
  const root = useWorkspaceStore((state) => state.workspaces[workspaceId]?.directory);
  const gitAvailable = useGitStore((state) => state.gitAvailable[workspaceId]);
  const repositories = useGitStore((state) => state.repositories[workspaceId] ?? EMPTY_ARRAY);
  const loadWorkspace = useGitStore((state) => state.loadWorkspace);

  useEffect(() => {
    if (root) void loadWorkspace(workspaceId, root);
  }, [workspaceId, root, loadWorkspace]);

  if (!root) return null;

  if (gitAvailable === undefined) {
    return <div className={styles.placeholder}>Checking for git…</div>;
  }

  if (!gitAvailable) {
    return (
      <div className={styles.placeholder}>
        <p>Git isn't installed, or isn't on your PATH.</p>
        <p>Install git to use source control features here.</p>
      </div>
    );
  }

  if (repositories.length === 0) {
    return <div className={styles.placeholder}>This folder isn't a git repository.</div>;
  }

  return (
    <div className={styles.gitPanel}>
      {repositories.map((repo) => (
        <RepositorySection key={repo.path} repo={repo} />
      ))}
    </div>
  );
}
