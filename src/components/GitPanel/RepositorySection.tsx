import { useState } from "react";
import { ArrowDown, ArrowUp, ChevronDown, ChevronRight, RefreshCw } from "lucide-react";

import { EMPTY_ARRAY } from "../../lib/emptyArray";
import { basename } from "../../lib/paths";
import type { FileStatusEntryDto, GitRepositoryDto } from "../../lib/types";
import { useGitStore } from "../../state/gitStore";
import styles from "./GitPanel.module.css";

function statusLetter(entry: FileStatusEntryDto): string {
  switch (entry.kind) {
    case "Modified":
      return "M";
    case "Added":
      return "A";
    case "Deleted":
      return "D";
    case "Renamed":
      return "R";
    case "Untracked":
      return "U";
    case "Conflicted":
      return "!";
  }
}

function FileRow({
  entry,
  repoPath,
  onStage,
  onUnstage,
}: {
  entry: FileStatusEntryDto;
  repoPath: string;
  onStage?: () => void;
  onUnstage?: () => void;
}) {
  const openDiff = useGitStore((state) => state.openDiff);
  return (
    <div
      className={styles.fileRow}
      onClick={() => openDiff({ repoPath, file: entry.path, staged: entry.staged })}
      title={entry.path}
    >
      <span className={`${styles.statusLetter} ${styles["status" + entry.kind]}`}>{statusLetter(entry)}</span>
      <span className={styles.fileRowLabel}>{basename(entry.path)}</span>
      {onStage && (
        <button
          type="button"
          className={styles.rowAction}
          onClick={(e) => {
            e.stopPropagation();
            onStage();
          }}
        >
          +
        </button>
      )}
      {onUnstage && (
        <button
          type="button"
          className={styles.rowAction}
          onClick={(e) => {
            e.stopPropagation();
            onUnstage();
          }}
        >
          −
        </button>
      )}
    </div>
  );
}

interface RepositorySectionProps {
  repo: GitRepositoryDto;
}

export function RepositorySection({ repo }: RepositorySectionProps) {
  const [expanded, setExpanded] = useState(true);
  const [historyExpanded, setHistoryExpanded] = useState(false);
  const status = useGitStore((state) => state.statuses[repo.path] ?? EMPTY_ARRAY);
  const branches = useGitStore((state) => state.branches[repo.path] ?? EMPTY_ARRAY);
  const currentBranch = useGitStore((state) => state.currentBranch[repo.path]);
  const sync = useGitStore((state) => state.sync[repo.path]);
  const commitMessage = useGitStore((state) => state.commitMessage[repo.path] ?? "");
  const history = useGitStore((state) => state.history[repo.path] ?? EMPTY_ARRAY);
  const historyComplete = useGitStore((state) => state.historyComplete[repo.path] ?? false);

  const refreshRepo = useGitStore((state) => state.refreshRepo);
  const stageFiles = useGitStore((state) => state.stageFiles);
  const unstageFiles = useGitStore((state) => state.unstageFiles);
  const setCommitMessage = useGitStore((state) => state.setCommitMessage);
  const commit = useGitStore((state) => state.commit);
  const switchBranch = useGitStore((state) => state.switchBranch);
  const push = useGitStore((state) => state.push);
  const pull = useGitStore((state) => state.pull);
  const fetch = useGitStore((state) => state.fetch);
  const loadMoreHistory = useGitStore((state) => state.loadMoreHistory);

  const staged = status.filter((entry) => entry.staged);
  const unstaged = status.filter((entry) => !entry.staged);

  return (
    <div className={styles.repo}>
      <div className={styles.repoHeader} onClick={() => setExpanded(!expanded)}>
        {expanded ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
        <span className={styles.repoName}>{repo.name}</span>
        <select
          className={styles.branchSelect}
          value={currentBranch ?? ""}
          onClick={(e) => e.stopPropagation()}
          onChange={(e) => void switchBranch(repo.path, e.target.value)}
        >
          {currentBranch && !branches.some((b) => b.name === currentBranch) && (
            <option value={currentBranch}>{currentBranch}</option>
          )}
          {branches.map((b) => (
            <option key={b.name} value={b.name}>
              {b.name}
            </option>
          ))}
        </select>
        {sync?.hasUpstream && (
          <span className={styles.syncCounts}>
            {sync.ahead > 0 && (
              <>
                <ArrowUp size={11} />
                {sync.ahead}
              </>
            )}
            {sync.behind > 0 && (
              <>
                <ArrowDown size={11} />
                {sync.behind}
              </>
            )}
          </span>
        )}
        <button
          type="button"
          className={styles.iconButton}
          title="Refresh"
          onClick={(e) => {
            e.stopPropagation();
            void refreshRepo(repo.path);
          }}
        >
          <RefreshCw size={13} />
        </button>
      </div>

      {expanded && (
        <div className={styles.repoBody}>
          <div className={styles.syncButtons}>
            <button type="button" onClick={() => void fetch(repo.path)}>
              Fetch
            </button>
            <button type="button" onClick={() => void pull(repo.path)}>
              Pull
            </button>
            <button type="button" onClick={() => void push(repo.path)}>
              Push
            </button>
          </div>

          <textarea
            className={styles.commitBox}
            placeholder="Commit message"
            value={commitMessage}
            onChange={(e) => setCommitMessage(repo.path, e.target.value)}
          />
          <button
            type="button"
            className={styles.commitButton}
            disabled={!commitMessage.trim() || staged.length === 0}
            onClick={() => void commit(repo.path)}
          >
            Commit ({staged.length})
          </button>

          {staged.length > 0 && (
            <div className={styles.section}>
              <div className={styles.sectionTitle}>Staged Changes</div>
              {staged.map((entry) => (
                <FileRow
                  key={entry.path}
                  entry={entry}
                  repoPath={repo.path}
                  onUnstage={() => void unstageFiles(repo.path, [entry.path])}
                />
              ))}
            </div>
          )}

          {unstaged.length > 0 && (
            <div className={styles.section}>
              <div className={styles.sectionTitle}>Changes</div>
              {unstaged.map((entry) => (
                <FileRow
                  key={entry.path}
                  entry={entry}
                  repoPath={repo.path}
                  onStage={() => void stageFiles(repo.path, [entry.path])}
                />
              ))}
            </div>
          )}

          {status.length === 0 && <div className={styles.emptyState}>No changes</div>}

          <div className={styles.section}>
            <div
              className={styles.sectionTitle}
              onClick={() => {
                setHistoryExpanded(!historyExpanded);
                if (!historyExpanded && history.length === 0) void loadMoreHistory(repo.path);
              }}
            >
              {historyExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />} History
            </div>
            {historyExpanded && (
              <div>
                {history.map((commitEntry) => (
                  <div key={commitEntry.hash} className={styles.commitRow} title={commitEntry.summary}>
                    <span className={styles.commitHash}>{commitEntry.shortHash}</span>
                    <span className={styles.commitSummary}>{commitEntry.summary}</span>
                  </div>
                ))}
                {!historyComplete && (
                  <button type="button" className={styles.loadMore} onClick={() => void loadMoreHistory(repo.path)}>
                    Load more
                  </button>
                )}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
