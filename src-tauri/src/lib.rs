mod commands;
mod events;
mod state;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use argus_application::use_cases::{
    ConfirmCloseWorkspaceUseCase, CreateWorkspaceUseCase, HandleProcessExitUseCase,
    RequestCloseWorkspaceUseCase, ResolveStartupPathUseCase,
};
use argus_application::WorkspaceManager;
use argus_infrastructure::{
    GitCliAdapter, NotifyWatcherAdapter, PlatformPathResolver, PortablePtyAdapter, StdFsAdapter,
    TauriDialogAdapter,
};
use tauri::Manager;

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(initial_directory: Option<PathBuf>) {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
            let pty = Arc::new(PortablePtyAdapter::new());
            let fs = Arc::new(StdFsAdapter::new());
            let watcher = Arc::new(NotifyWatcherAdapter::new());
            let git = Arc::new(GitCliAdapter::new());
            let picker = Arc::new(TauriDialogAdapter::new(app.handle().clone()));
            let path_resolver = Arc::new(PlatformPathResolver);
            let process_exit = Arc::new(HandleProcessExitUseCase::new(Arc::clone(&manager)));

            let state = AppState {
                manager: Arc::clone(&manager),
                pty: Arc::clone(&pty),
                fs,
                watcher,
                git,
                create_workspace: CreateWorkspaceUseCase::new(
                    Arc::clone(&manager),
                    Arc::clone(&pty),
                    picker,
                    process_exit,
                ),
                request_close_workspace: RequestCloseWorkspaceUseCase::new(Arc::clone(&manager)),
                confirm_close_workspace: ConfirmCloseWorkspaceUseCase::new(
                    Arc::clone(&manager),
                    Arc::clone(&pty),
                ),
                resolve_startup_path: ResolveStartupPathUseCase::new(
                    Arc::clone(&manager),
                    path_resolver,
                ),
                initial_directory,
            };

            app.manage(state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::resolve_startup_path,
            commands::get_initial_directory,
            commands::create_workspace_via_picker,
            commands::create_workspace_with_directory,
            commands::duplicate_workspace,
            commands::request_close_workspace,
            commands::confirm_close_workspace,
            commands::write_to_pty,
            commands::resize_pty,
            commands::list_workspaces,
            commands::get_shell_layout,
            commands::open_editor_panel,
            commands::list_dir,
            commands::read_file,
            commands::write_file,
            commands::create_file,
            commands::create_dir,
            commands::rename_path,
            commands::delete_path,
            commands::git_available,
            commands::git_list_repositories,
            commands::git_status,
            commands::git_diff,
            commands::git_stage,
            commands::git_unstage,
            commands::git_commit,
            commands::git_log,
            commands::git_current_branch,
            commands::git_list_branches,
            commands::git_switch_branch,
            commands::git_sync_status,
            commands::git_push,
            commands::git_pull,
            commands::git_fetch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
