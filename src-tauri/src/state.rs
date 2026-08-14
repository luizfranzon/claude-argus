use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use argus_application::use_cases::{
    ConfirmCloseSessionUseCase, ConfirmCloseWorkspaceUseCase, CreateSessionUseCase,
    CreateWorkspaceUseCase, RequestCloseSessionUseCase, RequestCloseWorkspaceUseCase,
    ResolveStartupPathUseCase,
};
use argus_application::WorkspaceManager;
use argus_infrastructure::{
    GitCliAdapter, HookServer, NotifyWatcherAdapter, PlatformPathResolver, PortablePtyAdapter,
    StdFsAdapter, TauriDialogAdapter,
};

pub type AppCreateSessionUseCase = CreateSessionUseCase<PortablePtyAdapter, HookServer>;
pub type AppCreateWorkspaceUseCase =
    CreateWorkspaceUseCase<PortablePtyAdapter, TauriDialogAdapter, HookServer>;
pub type AppConfirmCloseSessionUseCase = ConfirmCloseSessionUseCase<PortablePtyAdapter>;
pub type AppConfirmCloseWorkspaceUseCase = ConfirmCloseWorkspaceUseCase<PortablePtyAdapter>;
pub type AppResolveStartupPathUseCase = ResolveStartupPathUseCase<PlatformPathResolver>;

/// Composition root's assembled state: one instance per app process, built in
/// `run()`'s `.setup()` hook (once an `AppHandle` exists for the dialog
/// adapter) and registered as Tauri managed state.
pub struct AppState {
    pub manager: Arc<Mutex<WorkspaceManager>>,
    pub pty: Arc<PortablePtyAdapter>,
    /// Kept alive here so its background accept-loop thread keeps running
    /// for the lifetime of the app; never read directly elsewhere.
    #[allow(dead_code)]
    pub hook_server: Arc<HookServer>,
    pub fs: Arc<StdFsAdapter>,
    pub watcher: Arc<NotifyWatcherAdapter>,
    pub git: Arc<GitCliAdapter>,
    pub create_workspace: AppCreateWorkspaceUseCase,
    pub request_close_workspace: RequestCloseWorkspaceUseCase,
    pub confirm_close_workspace: AppConfirmCloseWorkspaceUseCase,
    pub create_session: Arc<AppCreateSessionUseCase>,
    pub request_close_session: RequestCloseSessionUseCase,
    pub confirm_close_session: AppConfirmCloseSessionUseCase,
    pub resolve_startup_path: AppResolveStartupPathUseCase,
    pub initial_directory: Option<PathBuf>,
}
