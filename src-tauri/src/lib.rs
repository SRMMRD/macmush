/// MACMush - A modern MUD/MUSH client for macOS
///
/// This is a faithful recreation of the Windows MUSHclient application,
/// built with Tauri 2.x, Rust, and modern web technologies.

// Module declarations
pub mod error;
pub mod core;
pub mod automation;
pub mod network;
pub mod scripting;
pub mod persistence;
pub mod ui;
pub mod world_file;

// Re-export commonly used types
pub use error::{MushError, Result};
pub use ui::state::AppState;
pub use ui::commands::*;

/// Initialize logging infrastructure
fn init_logging() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    tracing::info!("MACMush starting...");
}

/// Main application entry point
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();

    // Initialize application state
    let app_state = AppState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            connect_to_world,
            disconnect,
            send_command,
            add_trigger,
            get_connection_status,
            start_logging,
            stop_logging,
            write_log_entry,
            get_logging_status,
            open_logs_folder,
            import_world_file,
            export_world_file,
            get_tab_completions,
            execute_keypad_key,
            get_keypad_mappings,
            set_keypad_command,
            create_world,
            list_worlds,
            get_world,
            update_world,
            delete_world,
            create_timer,
            list_timers,
            get_timer,
            update_timer,
            delete_timer,
            create_alias,
            list_aliases,
            get_alias,
            update_alias,
            delete_alias,
            create_trigger,
            list_triggers,
            get_trigger,
            update_trigger,
            delete_trigger,
            create_highlight,
            list_highlights,
            get_highlight,
            update_highlight,
            delete_highlight,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_structure() {
        // Ensure all modules compile
        let _result: Result<()> = Ok(());
    }
}

