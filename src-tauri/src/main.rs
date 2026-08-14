// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let gui_launch = args.iter().any(|arg| arg == "--gui-launch");

    // CLI launch: trust the process's cwd (or an explicit directory argument).
    // GUI launch (desktop shortcut passes --gui-launch): no directory yet —
    // the frontend shows DirectoryPickerScreen instead.
    let initial_directory: Option<PathBuf> = if gui_launch {
        None
    } else {
        args.iter()
            .skip(1)
            .find(|arg| !arg.starts_with("--"))
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
    };

    argus_lib::run(initial_directory);
}
