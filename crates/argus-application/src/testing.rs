//! Fakes shared across use-case unit tests. Not compiled outside `cfg(test)` —
//! keeps `argus-application`'s real dependency tree free of test-only code.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::ports::{
    EnvResolutionError, ExitReason, FileSearchPort, HighlightedLines, HookCallbackPort, PtyError,
    PtyHandleId, PtyPort, SearchMatch, ShellEnvironmentResolver, SpawnSpec,
};

#[derive(Default)]
pub struct FakePtyPort {
    specs: Mutex<HashMap<PtyHandleId, SpawnSpec>>,
    spawned: Mutex<Vec<PtyHandleId>>,
    kill_calls: Mutex<Vec<PtyHandleId>>,
}

#[allow(dead_code)] // spawned_handles/trigger_output round out the fake's API for future tests
impl FakePtyPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawned_handles(&self) -> Vec<PtyHandleId> {
        self.spawned.lock().unwrap().clone()
    }

    pub fn kill_calls(&self) -> Vec<PtyHandleId> {
        self.kill_calls.lock().unwrap().clone()
    }

    /// Simulates the real PTY reader thread observing process exit.
    pub fn trigger_exit(&self, handle: PtyHandleId, reason: ExitReason) {
        let specs = self.specs.lock().unwrap();
        if let Some(spec) = specs.get(&handle) {
            (spec.on_exit)(reason);
        }
    }

    /// Simulates the real PTY reader thread delivering output bytes.
    pub fn trigger_output(&self, handle: PtyHandleId, data: Vec<u8>) {
        let specs = self.specs.lock().unwrap();
        if let Some(spec) = specs.get(&handle) {
            (spec.on_output)(data);
        }
    }

    /// The `args` of the most recently spawned `SpawnSpec`, if any.
    pub fn last_args(&self) -> Option<Vec<String>> {
        let spawned = self.spawned.lock().unwrap();
        let handle = spawned.last()?;
        self.specs.lock().unwrap().get(handle).map(|spec| spec.args.clone())
    }
}

#[async_trait]
impl PtyPort for FakePtyPort {
    async fn spawn(&self, spec: SpawnSpec) -> Result<PtyHandleId, PtyError> {
        let handle = PtyHandleId::new();
        self.spawned.lock().unwrap().push(handle);
        self.specs.lock().unwrap().insert(handle, spec);
        Ok(handle)
    }

    fn write(&self, _handle: PtyHandleId, _data: &[u8]) -> Result<(), PtyError> {
        Ok(())
    }

    fn resize(&self, _handle: PtyHandleId, _cols: u16, _rows: u16) -> Result<(), PtyError> {
        Ok(())
    }

    fn kill(&self, handle: PtyHandleId) -> Result<(), PtyError> {
        self.kill_calls.lock().unwrap().push(handle);
        Ok(())
    }
}

pub struct FakeShellEnvironmentResolver {
    result: Result<String, EnvResolutionError>,
    call_count: Mutex<u32>,
}

impl FakeShellEnvironmentResolver {
    pub fn returning(result: Result<String, EnvResolutionError>) -> Self {
        Self {
            result,
            call_count: Mutex::new(0),
        }
    }

    pub fn call_count(&self) -> u32 {
        *self.call_count.lock().unwrap()
    }
}

#[async_trait]
impl ShellEnvironmentResolver for FakeShellEnvironmentResolver {
    async fn resolve_path(&self) -> Result<String, EnvResolutionError> {
        *self.call_count.lock().unwrap() += 1;
        self.result.clone()
    }
}

/// Fake `FileSearchPort` for `SearchWorkspaceUseCase` tests — returns
/// canned results and records every call so assertions can check both.
#[derive(Default)]
pub struct FakeFileSearchPort {
    files: Vec<PathBuf>,
    matches: Vec<SearchMatch>,
    highlight: Option<HighlightedLines>,
    walk_calls: Mutex<Vec<(PathBuf, bool)>>,
}

impl FakeFileSearchPort {
    pub fn with_files(files: Vec<PathBuf>) -> Self {
        Self { files, ..Default::default() }
    }

    pub fn with_matches(matches: Vec<SearchMatch>) -> Self {
        Self { matches, ..Default::default() }
    }

    pub fn with_highlight(highlight: Option<HighlightedLines>) -> Self {
        Self { highlight, ..Default::default() }
    }

    pub fn walk_calls(&self) -> Vec<(PathBuf, bool)> {
        self.walk_calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl FileSearchPort for FakeFileSearchPort {
    async fn walk_files(&self, root: PathBuf, include_ignored: bool) -> Vec<PathBuf> {
        self.walk_calls.lock().unwrap().push((root, include_ignored));
        self.files.clone()
    }

    async fn grep_content(&self, _root: PathBuf, _query: String, _include_ignored: bool) -> Vec<SearchMatch> {
        self.matches.clone()
    }

    async fn highlight(&self, _path: PathBuf, _contents: String) -> Option<HighlightedLines> {
        self.highlight.clone()
    }
}

pub struct FakeHookCallbackPort {
    url: String,
}

impl FakeHookCallbackPort {
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }
}

impl HookCallbackPort for FakeHookCallbackPort {
    fn callback_url(&self) -> String {
        self.url.clone()
    }
}
