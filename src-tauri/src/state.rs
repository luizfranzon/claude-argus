use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use argus_application::use_cases::{
    ConfirmCloseWorkspaceUseCase, CreateWorkspaceUseCase, RequestCloseWorkspaceUseCase,
    ResolveStartupPathUseCase,
};
use argus_application::WorkspaceManager;
use argus_infrastructure::{
    GitCliAdapter, NotifyWatcherAdapter, PlatformPathResolver, PortablePtyAdapter, StdFsAdapter,
    TauriDialogAdapter,
};

pub type AppCreateWorkspaceUseCase = CreateWorkspaceUseCase<PortablePtyAdapter, TauriDialogAdapter>;
pub type AppConfirmCloseWorkspaceUseCase = ConfirmCloseWorkspaceUseCase<PortablePtyAdapter>;
pub type AppResolveStartupPathUseCase = ResolveStartupPathUseCase<PlatformPathResolver>;

/// Composition root's assembled state: one instance per app process, built in
/// `run()`'s `.setup()` hook (once an `AppHandle` exists for the dialog
/// adapter) and registered as Tauri managed state.
pub struct AppState {
    pub manager: Arc<Mutex<WorkspaceManager>>,
    pub pty: Arc<PortablePtyAdapter>,
    pub fs: Arc<StdFsAdapter>,
    pub watcher: Arc<NotifyWatcherAdapter>,
    pub git: Arc<GitCliAdapter>,
    pub create_workspace: AppCreateWorkspaceUseCase,
    pub request_close_workspace: RequestCloseWorkspaceUseCase,
    pub confirm_close_workspace: AppConfirmCloseWorkspaceUseCase,
    pub resolve_startup_path: AppResolveStartupPathUseCase,
    pub initial_directory: Option<PathBuf>,
}
