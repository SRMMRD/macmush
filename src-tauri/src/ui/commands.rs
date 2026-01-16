/// Tauri command functions
///
/// Provides the IPC bridge between frontend and backend Session management.

use crate::automation::triggers::{Trigger, TriggerAction};
use crate::automation::timers::{Timer, TimerAction, TimerType};
use crate::automation::aliases::{Alias, AliasAction};
use crate::automation::highlights::Highlight as AutoHighlight;
use crate::core::{Session, World};
use crate::ui::events::{start_data_receiver, start_event_forwarder};
use crate::ui::state::AppState;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Result type for Tauri commands (serializable error strings)
type CommandResult<T> = Result<T, String>;

/// Connection request from frontend
#[derive(Debug, Deserialize)]
pub struct ConnectRequest {
    pub name: String,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub use_tls: bool,
}

/// Trigger creation request from frontend
#[derive(Debug, Deserialize)]
pub struct AddTriggerRequest {
    pub name: String,
    pub pattern: String,
    pub command: String,
    #[serde(default)]
    pub script: Option<String>,
}

/// Connection status response
#[derive(Debug, Serialize)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub world_name: Option<String>,
    pub triggers_loaded: usize,
    pub aliases_loaded: usize,
    pub timers_loaded: usize,
    pub highlights_loaded: usize,
}

/// Connect to a MUD server
#[tauri::command]
pub async fn connect_to_world(
    request: ConnectRequest,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<ConnectionStatus> {
    info!(
        "Connect request: {} ({}:{})",
        request.name, request.host, request.port
    );

    // Check if already connected
    if state.is_connected().await {
        error!("Already connected to a MUD");
        return Err("Already connected. Disconnect first.".to_string());
    }

    // Create world configuration
    let world = World::new(&request.name, &request.host, request.port)
        .map_err(|e| format!("Invalid world configuration: {}", e))?;

    // Create new session
    let mut session = Session::new(world.clone(), state.event_bus.clone())
        .map_err(|e| format!("Failed to create session: {}", e))?;

    // Load persisted automation data into session
    let (triggers_loaded, aliases_loaded, timers_loaded, highlights_loaded) =
        load_automation_into_session(&mut session, &app_handle).await?;

    info!(
        "Loaded automation: {} triggers, {} aliases, {} timers, {} highlights",
        triggers_loaded, aliases_loaded, timers_loaded, highlights_loaded
    );

    // Attempt connection
    session
        .start()
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;

    info!("Successfully connected to {}", world.name);

    // Store session in state
    *state.session.lock().await = Some(session);

    // Start background event streaming
    info!("Starting event streaming to frontend");
    start_data_receiver(app_handle.clone(), state.inner().clone());
    start_event_forwarder(app_handle, state.inner().clone());

    Ok(ConnectionStatus {
        connected: true,
        world_name: Some(world.name),
        triggers_loaded,
        aliases_loaded,
        timers_loaded,
        highlights_loaded,
    })
}

/// Disconnect from current MUD server
#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>) -> CommandResult<ConnectionStatus> {
    info!("Disconnect request");

    // Get session and take ownership
    let mut session_guard = state.session.lock().await;
    let mut session = session_guard.take();

    match session.as_mut() {
        Some(session) => {
            // Disconnect
            session
                .stop()
                .await
                .map_err(|e| format!("Disconnect failed: {}", e))?;

            info!("Successfully disconnected");

            Ok(ConnectionStatus {
                connected: false,
                world_name: None,
                triggers_loaded: 0,
                aliases_loaded: 0,
                timers_loaded: 0,
                highlights_loaded: 0,
            })
        }
        None => {
            debug!("Not connected");
            Err("Not connected".to_string())
        }
    }
}

/// Send command to MUD server
#[tauri::command]
pub async fn send_command(command: String, state: State<'_, AppState>) -> CommandResult<()> {
    debug!("Send command: {}", command);

    // Hold lock and send command
    let mut session_guard = state.session.lock().await;

    match session_guard.as_mut() {
        Some(session) => {
            session
                .send_command(&command)
                .await
                .map_err(|e| format!("Failed to send command: {}", e))
        }
        None => Err("Not connected".to_string()),
    }
}

/// Add trigger to current session
#[tauri::command]
pub async fn add_trigger(
    request: AddTriggerRequest,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    info!("Add trigger: {} -> {}", request.pattern, request.command);

    // Get session
    let mut session_guard = state.session.lock().await;

    match session_guard.as_mut() {
        Some(session) => {
            // Determine action based on script vs command
            let action = if let Some(script) = request.script {
                if !script.trim().is_empty() {
                    TriggerAction::ExecuteScript(script)
                } else {
                    TriggerAction::SendCommand(request.command)
                }
            } else {
                TriggerAction::SendCommand(request.command)
            };

            // Create trigger
            let trigger = Trigger::new(
                &request.name,
                &request.pattern,
                action,
            )
            .map_err(|e| format!("Invalid trigger: {}", e))?;

            // Add to session
            session
                .add_trigger(trigger)
                .map_err(|e| format!("Failed to add trigger: {}", e))?;

            Ok(())
        }
        None => Err("Not connected".to_string()),
    }
}

/// Get connection status
#[tauri::command]
pub async fn get_connection_status(
    state: State<'_, AppState>,
) -> CommandResult<ConnectionStatus> {
    let session_guard = state.session.lock().await;

    match session_guard.as_ref() {
        Some(session) => Ok(ConnectionStatus {
            connected: session.is_connected(),
            world_name: Some(session.world().name.clone()),
            triggers_loaded: 0,
            aliases_loaded: 0,
            timers_loaded: 0,
            highlights_loaded: 0,
        }),
        None => Ok(ConnectionStatus {
            connected: false,
            world_name: None,
            triggers_loaded: 0,
            aliases_loaded: 0,
            timers_loaded: 0,
            highlights_loaded: 0,
        }),
    }
}

/// Logging request from frontend
#[derive(Debug, Deserialize)]
pub struct StartLoggingRequest {
    pub world_name: String,
    pub format: String, // 'plain', 'html', 'raw'
}

/// Logging status response
#[derive(Debug, Serialize)]
pub struct LoggingStatus {
    pub is_logging: bool,
    pub log_file: Option<String>,
}

/// Get logs directory path
fn get_logs_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    let logs_dir = app_data_dir.join("logs");

    // Create logs directory if it doesn't exist
    fs::create_dir_all(&logs_dir)
        .map_err(|e| format!("Failed to create logs directory: {}", e))?;

    Ok(logs_dir)
}

/// Generate log filename with timestamp
fn generate_log_filename(world_name: &str, format: &str) -> String {
    let now = chrono::Local::now();
    let timestamp = now.format("%Y%m%d-%H%M%S");
    let extension = match format {
        "html" => "html",
        "raw" => "txt",
        _ => "log",
    };
    format!("{}_{}.{}", world_name, timestamp, extension)
}

/// Start logging session output to file
#[tauri::command]
pub async fn start_logging(
    request: StartLoggingRequest,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<LoggingStatus> {
    info!("Start logging: {} ({})", request.world_name, request.format);

    // Get logs directory
    let logs_dir = get_logs_dir(&app_handle)?;

    // Generate log filename
    let filename = generate_log_filename(&request.world_name, &request.format);
    let log_path = logs_dir.join(&filename);

    // Create log file
    let mut file = File::create(&log_path)
        .map_err(|e| format!("Failed to create log file: {}", e))?;

    // Write header based on format
    match request.format.as_str() {
        "html" => {
            writeln!(file, "<!DOCTYPE html>")
                .and_then(|_| writeln!(file, "<html>"))
                .and_then(|_| writeln!(file, "<head>"))
                .and_then(|_| writeln!(file, "<meta charset=\"UTF-8\">"))
                .and_then(|_| writeln!(file, "<title>{} - Log Session</title>", request.world_name))
                .and_then(|_| writeln!(file, "<style>"))
                .and_then(|_| writeln!(file, "body {{ background: #000; color: #fff; font-family: 'Courier New', monospace; padding: 20px; }}"))
                .and_then(|_| writeln!(file, ".timestamp {{ color: #888; }}"))
                .and_then(|_| writeln!(file, ".command {{ color: #0f0; }}"))
                .and_then(|_| writeln!(file, ".system {{ color: #ff0; }}"))
                .and_then(|_| writeln!(file, ".error {{ color: #f00; }}"))
                .and_then(|_| writeln!(file, "</style>"))
                .and_then(|_| writeln!(file, "</head>"))
                .and_then(|_| writeln!(file, "<body>"))
                .and_then(|_| writeln!(file, "<h1>MACMush Log: {}</h1>", request.world_name))
                .and_then(|_| writeln!(file, "<p>Started: {}</p><hr>", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")))
                .and_then(|_| writeln!(file, "<pre>"))
                .map_err(|e| format!("Failed to write HTML header: {}", e))?;
        }
        _ => {
            writeln!(file, "=== MACMush Log Session ===")
                .and_then(|_| writeln!(file, "World: {}", request.world_name))
                .and_then(|_| writeln!(file, "Started: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")))
                .and_then(|_| writeln!(file, "Format: {}", request.format))
                .and_then(|_| writeln!(file, "==============================\n"))
                .map_err(|e| format!("Failed to write log header: {}", e))?;
        }
    }

    // Store log file path in state
    *state.log_file.lock().await = Some(log_path.to_string_lossy().to_string());
    *state.log_format.lock().await = request.format.clone();

    info!("Logging started to: {:?}", log_path);

    Ok(LoggingStatus {
        is_logging: true,
        log_file: Some(filename),
    })
}

/// Stop logging session
#[tauri::command]
pub async fn stop_logging(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<LoggingStatus> {
    info!("Stop logging");

    let mut log_file_guard = state.log_file.lock().await;
    let log_format_guard = state.log_format.lock().await;

    if let Some(log_path_str) = log_file_guard.as_ref() {
        let log_path = PathBuf::from(log_path_str);

        // Write footer based on format
        if let Ok(mut file) = OpenOptions::new().append(true).open(&log_path) {
            match log_format_guard.as_str() {
                "html" => {
                    let _ = writeln!(file, "</pre>");
                    let _ = writeln!(file, "<hr>");
                    let _ = writeln!(file, "<p>Ended: {}</p>", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
                    let _ = writeln!(file, "</body>");
                    let _ = writeln!(file, "</html>");
                }
                _ => {
                    let _ = writeln!(file, "\n==============================");
                    let _ = writeln!(file, "Ended: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
                    let _ = writeln!(file, "==============================");
                }
            }
        }
    }

    *log_file_guard = None;

    info!("Logging stopped");

    Ok(LoggingStatus {
        is_logging: false,
        log_file: None,
    })
}

/// Write entry to log file
#[tauri::command]
pub async fn write_log_entry(
    text: String,
    message_type: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let log_file_guard = state.log_file.lock().await;
    let log_format_guard = state.log_format.lock().await;

    if let Some(log_path_str) = log_file_guard.as_ref() {
        let log_path = PathBuf::from(log_path_str);

        if let Ok(mut file) = OpenOptions::new().append(true).open(log_path) {
            let timestamp = chrono::Local::now().format("%H:%M:%S");

            let result = match log_format_guard.as_str() {
                "html" => {
                    let class = match message_type.as_str() {
                        "command" => "command",
                        "system" => "system",
                        "error" => "error",
                        _ => "",
                    };
                    writeln!(
                        file,
                        "<span class=\"timestamp\">[{}]</span> <span class=\"{}\">{}</span>",
                        timestamp, class, html_escape::encode_text(&text)
                    )
                }
                "raw" => {
                    // Include ANSI codes if present
                    writeln!(file, "{}", text)
                }
                _ => {
                    // Plain text with timestamp
                    writeln!(file, "[{}] {}", timestamp, text)
                }
            };

            result.map_err(|e| format!("Failed to write log entry: {}", e))?;
        }
    }

    Ok(())
}

/// Get logging status
#[tauri::command]
pub async fn get_logging_status(state: State<'_, AppState>) -> CommandResult<LoggingStatus> {
    let log_file_guard = state.log_file.lock().await;

    Ok(LoggingStatus {
        is_logging: log_file_guard.is_some(),
        log_file: log_file_guard.as_ref().map(|p| {
            PathBuf::from(p)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        }),
    })
}

/// Open logs folder in file manager
#[tauri::command]
pub async fn open_logs_folder(app_handle: AppHandle) -> CommandResult<()> {
    let logs_dir = get_logs_dir(&app_handle)?;

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&logs_dir)
            .spawn()
            .map_err(|e| format!("Failed to open logs folder: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&logs_dir)
            .spawn()
            .map_err(|e| format!("Failed to open logs folder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&logs_dir)
            .spawn()
            .map_err(|e| format!("Failed to open logs folder: {}", e))?;
    }

    Ok(())
}

/// Import world file from XML
#[tauri::command]
pub async fn import_world_file(file_path: String) -> CommandResult<ImportResult> {
    use crate::world_file::WorldFile;
    use std::fs;

    info!("Importing world file from: {}", file_path);

    // Read XML file
    let xml_content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read world file: {}", e))?;

    // Parse XML
    let world_file = WorldFile::from_xml(&xml_content)
        .map_err(|e| format!("Failed to parse world file: {}", e))?;

    // Count imported items
    let trigger_count = world_file.triggers.as_ref().map(|t| t.items.len()).unwrap_or(0);
    let alias_count = world_file.aliases.as_ref().map(|a| a.items.len()).unwrap_or(0);
    let timer_count = world_file.timers.as_ref().map(|t| t.items.len()).unwrap_or(0);
    let macro_count = world_file.macros.as_ref().map(|m| m.items.len()).unwrap_or(0);
    let variable_count = world_file.variables.as_ref().map(|v| v.items.len()).unwrap_or(0);

    info!(
        "Imported {} triggers, {} aliases, {} timers, {} macros, {} variables",
        trigger_count, alias_count, timer_count, macro_count, variable_count
    );

    Ok(ImportResult {
        success: true,
        world_name: world_file.world.as_ref().and_then(|w| w.name.clone()),
        trigger_count,
        alias_count,
        timer_count,
        macro_count,
        variable_count,
        world_file: serde_json::to_string(&world_file)
            .map_err(|e| format!("Failed to serialize world file: {}", e))?,
    })
}

/// Export world file to XML
#[tauri::command]
pub async fn export_world_file(
    file_path: String,
    world_data: String,
) -> CommandResult<ExportResult> {
    use crate::world_file::WorldFile;
    use std::fs;

    info!("Exporting world file to: {}", file_path);

    // Parse world data
    let world_file: WorldFile = serde_json::from_str(&world_data)
        .map_err(|e| format!("Failed to parse world data: {}", e))?;

    // Convert to XML
    let xml_content = world_file.to_xml()
        .map_err(|e| format!("Failed to generate XML: {}", e))?;

    // Write to file
    fs::write(&file_path, xml_content)
        .map_err(|e| format!("Failed to write world file: {}", e))?;

    info!("World file exported successfully");

    Ok(ExportResult {
        success: true,
        file_path,
    })
}

/// Get tab-completion matches for partial input
#[tauri::command]
pub async fn get_tab_completions(
    partial: String,
    state: State<'_, AppState>,
) -> CommandResult<Vec<String>> {
    let session_guard = state.session.lock().await;

    if let Some(session) = session_guard.as_ref() {
        let completions = session.get_completions(&partial);
        Ok(completions)
    } else {
        // No active session, return empty
        Ok(Vec::new())
    }
}

/// Keypad key press request from frontend
#[derive(Debug, Deserialize)]
pub struct KeypadPressRequest {
    pub key: String,      // "0"-"9", "dot", "slash", "star", "minus", "plus", "enter"
    pub ctrl: bool,       // Ctrl key modifier
}

/// Execute keypad key press
#[tauri::command]
pub async fn execute_keypad_key(
    request: KeypadPressRequest,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    use crate::automation::{KeypadKey, KeypadModifier};

    let mut session_guard = state.session.lock().await;

    if let Some(session) = session_guard.as_mut() {
        // Parse keypad key from string
        let key = KeypadKey::from_str(&request.key)
            .ok_or_else(|| format!("Invalid keypad key: {}", request.key))?;

        // Determine modifier
        let modifier = if request.ctrl {
            KeypadModifier::Ctrl
        } else {
            KeypadModifier::None
        };

        // Execute the keypad command
        session
            .execute_keypad_key(key, modifier)
            .await
            .map_err(|e| format!("Failed to execute keypad key: {}", e))?;

        Ok(())
    } else {
        Err("No active session".to_string())
    }
}

/// Get all keypad mappings for configuration UI
#[derive(Debug, Serialize)]
pub struct KeypadMappings {
    pub normal: Vec<KeypadMapping>,
    pub ctrl: Vec<KeypadMapping>,
}

#[derive(Debug, Serialize)]
pub struct KeypadMapping {
    pub key: String,
    pub command: String,
}

#[tauri::command]
pub async fn get_keypad_mappings(
    state: State<'_, AppState>,
) -> CommandResult<KeypadMappings> {
    use crate::automation::KeypadKey;

    let session_guard = state.session.lock().await;

    if let Some(session) = session_guard.as_ref() {
        let keypad = session.keypad_mapping();

        // Convert normal mappings
        let normal: Vec<KeypadMapping> = keypad
            .get_normal_mappings()
            .iter()
            .map(|(key, command)| KeypadMapping {
                key: key.to_string().to_string(),
                command: command.clone(),
            })
            .collect();

        // Convert Ctrl mappings
        let ctrl: Vec<KeypadMapping> = keypad
            .get_ctrl_mappings()
            .iter()
            .map(|(key, command)| KeypadMapping {
                key: key.to_string().to_string(),
                command: command.clone(),
            })
            .collect();

        Ok(KeypadMappings { normal, ctrl })
    } else {
        Err("No active session".to_string())
    }
}

/// Set keypad command request from frontend
#[derive(Debug, Deserialize)]
pub struct SetKeypadCommandRequest {
    pub key: String,      // "0"-"9", "dot", "slash", "star", "minus", "plus", "enter"
    pub ctrl: bool,       // Ctrl key modifier
    pub command: String,  // Command to execute
}

#[tauri::command]
pub async fn set_keypad_command(
    request: SetKeypadCommandRequest,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    use crate::automation::{KeypadKey, KeypadModifier};

    let mut session_guard = state.session.lock().await;

    if let Some(session) = session_guard.as_mut() {
        // Parse keypad key from string
        let key = KeypadKey::from_str(&request.key)
            .ok_or_else(|| format!("Invalid keypad key: {}", request.key))?;

        // Determine modifier
        let modifier = if request.ctrl {
            KeypadModifier::Ctrl
        } else {
            KeypadModifier::None
        };

        // Set the command
        session
            .keypad_mapping_mut()
            .set_command(key, modifier, request.command);

        info!("Set keypad command for {}+{}", if request.ctrl { "Ctrl" } else { "" }, key.to_string());
        Ok(())
    } else {
        Err("No active session".to_string())
    }
}

/// Import result response
#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub success: bool,
    pub world_name: Option<String>,
    pub trigger_count: usize,
    pub alias_count: usize,
    pub timer_count: usize,
    pub macro_count: usize,
    pub variable_count: usize,
    pub world_file: String,
}

/// Export result response
#[derive(Debug, Serialize)]
pub struct ExportResult {
    pub success: bool,
    pub file_path: String,
}

// ============================================================================
// World Management Commands
// ============================================================================

/// Create world request
#[derive(Debug, Deserialize)]
pub struct CreateWorldRequest {
    pub name: String,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub auto_connect: bool,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub use_tls: bool,
}

fn default_timeout() -> u64 {
    30
}

/// Update world request
#[derive(Debug, Deserialize)]
pub struct UpdateWorldRequest {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub auto_connect: bool,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub use_tls: bool,
}

/// Get worlds directory path
fn get_worlds_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    let worlds_dir = app_data_dir.join("worlds");

    // Create worlds directory if it doesn't exist
    fs::create_dir_all(&worlds_dir)
        .map_err(|e| format!("Failed to create worlds directory: {}", e))?;

    Ok(worlds_dir)
}

/// Create a new world configuration
#[tauri::command]
pub async fn create_world(
    request: CreateWorldRequest,
    app_handle: AppHandle,
) -> CommandResult<World> {
    info!("Creating world: {}", request.name);

    // Build world configuration
    let world = World::builder(&request.name, &request.host, request.port)
        .description(request.description.unwrap_or_default())
        .auto_connect(request.auto_connect)
        .timeout_secs(request.timeout_secs)
        .use_tls(request.use_tls)
        .build()
        .map_err(|e| format!("Failed to create world: {}", e))?;

    // Get worlds directory
    let worlds_dir = get_worlds_dir(&app_handle)?;

    // Save world to file
    let world_file = worlds_dir.join(format!("{}.json", world.id));
    let json = serde_json::to_string_pretty(&world)
        .map_err(|e| format!("Failed to serialize world: {}", e))?;

    fs::write(&world_file, json)
        .map_err(|e| format!("Failed to write world file: {}", e))?;

    info!("World '{}' created with ID {}", world.name, world.id);

    Ok(world)
}

/// List all saved worlds
#[tauri::command]
pub async fn list_worlds(app_handle: AppHandle) -> CommandResult<Vec<World>> {
    info!("Listing worlds");

    // Get worlds directory
    let worlds_dir = get_worlds_dir(&app_handle)?;

    // Read all .json files in worlds directory
    let entries = fs::read_dir(&worlds_dir)
        .map_err(|e| format!("Failed to read worlds directory: {}", e))?;

    let mut worlds = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            match fs::read_to_string(&path) {
                Ok(json) => match serde_json::from_str::<World>(&json) {
                    Ok(world) => worlds.push(world),
                    Err(e) => warn!("Failed to parse world file {:?}: {}", path, e),
                },
                Err(e) => warn!("Failed to read world file {:?}: {}", path, e),
            }
        }
    }

    info!("Found {} worlds", worlds.len());

    Ok(worlds)
}

/// Get a specific world by ID
#[tauri::command]
pub async fn get_world(id: String, app_handle: AppHandle) -> CommandResult<World> {
    info!("Getting world: {}", id);

    // Get worlds directory
    let worlds_dir = get_worlds_dir(&app_handle)?;

    // Read world file
    let world_file = worlds_dir.join(format!("{}.json", id));
    let json = fs::read_to_string(&world_file)
        .map_err(|e| format!("World not found: {}", e))?;

    let world: World = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse world: {}", e))?;

    Ok(world)
}

/// Update an existing world configuration
#[tauri::command]
pub async fn update_world(
    request: UpdateWorldRequest,
    app_handle: AppHandle,
) -> CommandResult<World> {
    info!("Updating world: {}", request.id);

    // Parse UUID
    let id = Uuid::parse_str(&request.id)
        .map_err(|e| format!("Invalid world ID: {}", e))?;

    // Build updated world configuration
    let world = World::builder(&request.name, &request.host, request.port)
        .id(id)
        .description(request.description.unwrap_or_default())
        .auto_connect(request.auto_connect)
        .timeout_secs(request.timeout_secs)
        .use_tls(request.use_tls)
        .build()
        .map_err(|e| format!("Failed to update world: {}", e))?;

    // Get worlds directory
    let worlds_dir = get_worlds_dir(&app_handle)?;

    // Save world to file
    let world_file = worlds_dir.join(format!("{}.json", world.id));
    let json = serde_json::to_string_pretty(&world)
        .map_err(|e| format!("Failed to serialize world: {}", e))?;

    fs::write(&world_file, json)
        .map_err(|e| format!("Failed to write world file: {}", e))?;

    info!("World '{}' updated", world.name);

    Ok(world)
}

/// Delete a world configuration
#[tauri::command]
pub async fn delete_world(id: String, app_handle: AppHandle) -> CommandResult<()> {
    info!("Deleting world: {}", id);

    // Get worlds directory
    let worlds_dir = get_worlds_dir(&app_handle)?;

    // Delete world file
    let world_file = worlds_dir.join(format!("{}.json", id));
    fs::remove_file(&world_file)
        .map_err(|e| format!("Failed to delete world: {}", e))?;

    info!("World {} deleted", id);

    Ok(())
}

// ============================================================================
// Automation Loading Helper
// ============================================================================

/// Load all persisted automation data into a session
/// This is called during world connection to restore triggers, aliases, timers, and highlights
pub async fn load_automation_into_session(
    session: &mut Session,
    app_handle: &AppHandle,
) -> Result<(usize, usize, usize, usize), String> {
    info!("Loading persisted automation data into session");

    let mut triggers_loaded = 0;
    let mut aliases_loaded = 0;
    let mut timers_loaded = 0;
    let mut highlights_loaded = 0;

    // Load triggers
    let triggers_dir = get_triggers_dir(app_handle)?;
    if triggers_dir.exists() {
        if let Ok(entries) = fs::read_dir(&triggers_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(trigger) = serde_json::from_str::<Trigger>(&content) {
                            if let Ok(_) = session.add_trigger(trigger) {
                                triggers_loaded += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    // Load aliases
    let aliases_dir = get_aliases_dir(app_handle)?;
    if aliases_dir.exists() {
        if let Ok(entries) = fs::read_dir(&aliases_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(alias) = serde_json::from_str::<Alias>(&content) {
                            if let Ok(_) = session.add_alias(alias) {
                                aliases_loaded += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    // Load timers
    let timers_dir = get_timers_dir(app_handle)?;
    if timers_dir.exists() {
        if let Ok(entries) = fs::read_dir(&timers_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(timer) = serde_json::from_str::<Timer>(&content) {
                            if let Ok(_) = session.add_timer(timer) {
                                timers_loaded += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    // Load highlights
    use crate::automation::Highlight;
    let highlights_dir = get_highlights_dir(app_handle)?;
    if highlights_dir.exists() {
        if let Ok(entries) = fs::read_dir(&highlights_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(highlight) = serde_json::from_str::<Highlight>(&content) {
                            if let Ok(_) = session.add_highlight(highlight) {
                                highlights_loaded += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    info!(
        "Loaded automation: {} triggers, {} aliases, {} timers, {} highlights",
        triggers_loaded, aliases_loaded, timers_loaded, highlights_loaded
    );

    Ok((triggers_loaded, aliases_loaded, timers_loaded, highlights_loaded))
}

// ============================================================================
// Timer Management Commands
// ============================================================================

/// Timer creation request
#[derive(Debug, Deserialize)]
pub struct CreateTimerRequest {
    pub name: String,
    pub timer_type: String, // "oneshot" or "repeating"
    pub interval_secs: f64,
    pub action: String, // "send_command", "send_commands", "execute_script"
    pub command: Option<String>,
    pub commands: Option<Vec<String>>,
    pub script: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Timer update request
#[derive(Debug, Deserialize)]
pub struct UpdateTimerRequest {
    pub id: String,
    pub name: Option<String>,
    pub timer_type: Option<String>,
    pub interval_secs: Option<f64>,
    pub action: Option<String>,
    pub command: Option<String>,
    pub commands: Option<Vec<String>>,
    pub script: Option<String>,
    pub enabled: Option<bool>,
}

fn default_true() -> bool {
    true
}

/// Get timers directory path
fn get_timers_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    let timers_dir = app_data_dir.join("timers");
    fs::create_dir_all(&timers_dir)
        .map_err(|e| format!("Failed to create timers directory: {}", e))?;
    Ok(timers_dir)
}

/// Create a new timer
#[tauri::command]
pub async fn create_timer(request: CreateTimerRequest, app_handle: AppHandle) -> CommandResult<Timer> {
    info!("Creating timer: {}", request.name);

    // Parse timer type
    let timer_type = match request.timer_type.to_lowercase().as_str() {
        "oneshot" => TimerType::OneShot,
        "repeating" => TimerType::Repeating,
        _ => return Err("Invalid timer type. Must be 'oneshot' or 'repeating'".to_string()),
    };

    // Parse action
    let action = match request.action.to_lowercase().as_str() {
        "send_command" => {
            let cmd = request.command.ok_or("command field required for send_command action")?;
            TimerAction::SendCommand(cmd)
        }
        "send_commands" => {
            let cmds = request.commands.ok_or("commands field required for send_commands action")?;
            TimerAction::SendCommands(cmds)
        }
        "execute_script" => {
            let script = request.script.ok_or("script field required for execute_script action")?;
            TimerAction::ExecuteScript(script)
        }
        _ => return Err("Invalid action type".to_string()),
    };

    // Create timer
    let mut timer = Timer::new(request.name, timer_type, request.interval_secs, action)
        .map_err(|e| format!("Failed to create timer: {}", e))?;

    timer.enabled = request.enabled;

    // Get timers directory
    let timers_dir = get_timers_dir(&app_handle)?;

    // Save timer to file
    let timer_file = timers_dir.join(format!("{}.json", timer.id));
    let json = serde_json::to_string_pretty(&timer)
        .map_err(|e| format!("Failed to serialize timer: {}", e))?;

    fs::write(&timer_file, json)
        .map_err(|e| format!("Failed to write timer file: {}", e))?;

    info!("Timer '{}' created with ID {}", timer.name, timer.id);

    // Sync with active session if connected
    let state: tauri::State<AppState> = app_handle.state();
    let mut session_guard = state.session.lock().await;
    if let Some(session) = session_guard.as_mut() {
        session.add_timer(timer.clone())
            .map_err(|e| format!("Failed to add timer to session: {}", e))?;
        info!("Timer synced to active session");
    }

    Ok(timer)
}

/// List all timers
#[tauri::command]
pub async fn list_timers(app_handle: AppHandle) -> CommandResult<Vec<Timer>> {
    debug!("Listing all timers");

    let timers_dir = get_timers_dir(&app_handle)?;
    let mut timers = Vec::new();

    // Read all JSON files in timers directory
    let entries = fs::read_dir(&timers_dir)
        .map_err(|e| format!("Failed to read timers directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read timer file: {}", e))?;

            let timer: Timer = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse timer: {}", e))?;

            timers.push(timer);
        }
    }

    debug!("Found {} timer(s)", timers.len());
    Ok(timers)
}

/// Get a specific timer by ID
#[tauri::command]
pub async fn get_timer(id: String, app_handle: AppHandle) -> CommandResult<Timer> {
    debug!("Getting timer: {}", id);

    let timers_dir = get_timers_dir(&app_handle)?;
    let timer_file = timers_dir.join(format!("{}.json", id));

    if !timer_file.exists() {
        return Err(format!("Timer not found: {}", id));
    }

    let content = fs::read_to_string(&timer_file)
        .map_err(|e| format!("Failed to read timer file: {}", e))?;

    let timer: Timer = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse timer: {}", e))?;

    Ok(timer)
}

/// Update a timer
#[tauri::command]
pub async fn update_timer(request: UpdateTimerRequest, app_handle: AppHandle) -> CommandResult<Timer> {
    info!("Updating timer: {}", request.id);

    // Get existing timer
    let mut timer = get_timer(request.id.clone(), app_handle.clone()).await?;

    // Update fields
    if let Some(name) = request.name {
        timer.name = name;
    }

    if let Some(timer_type) = request.timer_type {
        timer.timer_type = match timer_type.to_lowercase().as_str() {
            "oneshot" => TimerType::OneShot,
            "repeating" => TimerType::Repeating,
            _ => return Err("Invalid timer type".to_string()),
        };
    }

    if let Some(interval_secs) = request.interval_secs {
        timer.interval = interval_secs;
    }

    if let Some(action) = request.action {
        timer.action = match action.to_lowercase().as_str() {
            "send_command" => {
                let cmd = request.command.ok_or("command field required")?;
                TimerAction::SendCommand(cmd)
            }
            "send_commands" => {
                let cmds = request.commands.ok_or("commands field required")?;
                TimerAction::SendCommands(cmds)
            }
            "execute_script" => {
                let script = request.script.ok_or("script field required")?;
                TimerAction::ExecuteScript(script)
            }
            _ => return Err("Invalid action type".to_string()),
        };
    }

    if let Some(enabled) = request.enabled {
        timer.enabled = enabled;
    }

    // Get timers directory
    let timers_dir = get_timers_dir(&app_handle)?;

    // Save timer to file
    let timer_file = timers_dir.join(format!("{}.json", timer.id));
    let json = serde_json::to_string_pretty(&timer)
        .map_err(|e| format!("Failed to serialize timer: {}", e))?;

    fs::write(&timer_file, json)
        .map_err(|e| format!("Failed to write timer file: {}", e))?;

    info!("Timer '{}' updated in file", timer.name);

    // Sync with active session if connected
    let state: tauri::State<AppState> = app_handle.state();
    let mut session_guard = state.session.lock().await;
    if let Some(session) = session_guard.as_mut() {
        session.update_timer(timer.clone())
            .map_err(|e| format!("Failed to update timer in session: {}", e))?;
        info!("Timer updated in active session");
    }

    Ok(timer)
}

/// Delete a timer
#[tauri::command]
pub async fn delete_timer(id: String, app_handle: AppHandle) -> CommandResult<()> {
    info!("Deleting timer: {}", id);

    let timer_id = Uuid::parse_str(&id)
        .map_err(|e| format!("Invalid timer ID: {}", e))?;

    let timers_dir = get_timers_dir(&app_handle)?;
    let timer_file = timers_dir.join(format!("{}.json", id));

    fs::remove_file(&timer_file)
        .map_err(|e| format!("Failed to delete timer: {}", e))?;

    info!("Timer {} deleted from file", id);

    // Remove from active session if connected
    let state: tauri::State<AppState> = app_handle.state();
    let mut session_guard = state.session.lock().await;
    if let Some(session) = session_guard.as_mut() {
        session.remove_timer(timer_id)
            .map_err(|e| format!("Failed to remove timer from session: {}", e))?;
        info!("Timer removed from active session");
    }

    Ok(())
}

// ============================================================================
// Alias Management Commands
// ============================================================================

/// Alias creation request
#[derive(Debug, Deserialize)]
pub struct CreateAliasRequest {
    pub name: String,
    pub pattern: String,
    pub action: String, // "send_command", "send_commands", "execute_script"
    pub command: Option<String>,
    pub commands: Option<Vec<String>>,
    pub script: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Alias update request
#[derive(Debug, Deserialize)]
pub struct UpdateAliasRequest {
    pub id: String,
    pub name: Option<String>,
    pub pattern: Option<String>,
    pub action: Option<String>,
    pub command: Option<String>,
    pub commands: Option<Vec<String>>,
    pub script: Option<String>,
    pub enabled: Option<bool>,
}

/// Get aliases directory path
fn get_aliases_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    let aliases_dir = app_data_dir.join("aliases");
    fs::create_dir_all(&aliases_dir)
        .map_err(|e| format!("Failed to create aliases directory: {}", e))?;
    Ok(aliases_dir)
}

/// Create a new alias
#[tauri::command]
pub async fn create_alias(request: CreateAliasRequest, app_handle: AppHandle) -> CommandResult<Alias> {
    info!("Creating alias: {}", request.name);

    // Parse action
    let action = match request.action.to_lowercase().as_str() {
        "send_command" => {
            let cmd = request.command.ok_or("command field required for send_command action")?;
            AliasAction::SendCommand(cmd)
        }
        "send_commands" => {
            let cmds = request.commands.ok_or("commands field required for send_commands action")?;
            AliasAction::SendCommands(cmds)
        }
        "execute_script" => {
            let script = request.script.ok_or("script field required for execute_script action")?;
            AliasAction::ExecuteScript(script)
        }
        _ => return Err("Invalid action type".to_string()),
    };

    // Create alias
    let mut alias = Alias::new(request.name, request.pattern, action)
        .map_err(|e| format!("Failed to create alias: {}", e))?;

    alias.enabled = request.enabled;

    // Get aliases directory
    let aliases_dir = get_aliases_dir(&app_handle)?;

    // Save alias to file
    let alias_file = aliases_dir.join(format!("{}.json", alias.id));
    let json = serde_json::to_string_pretty(&alias)
        .map_err(|e| format!("Failed to serialize alias: {}", e))?;

    fs::write(&alias_file, json)
        .map_err(|e| format!("Failed to write alias file: {}", e))?;

    info!("Alias '{}' created with ID {}", alias.name, alias.id);

    // Sync with active session if connected
    let state: tauri::State<AppState> = app_handle.state();
    let mut session_guard = state.session.lock().await;
    if let Some(session) = session_guard.as_mut() {
        session.add_alias(alias.clone())
            .map_err(|e| format!("Failed to add alias to session: {}", e))?;
        info!("Alias synced to active session");
    }

    Ok(alias)
}

/// List all aliases
#[tauri::command]
pub async fn list_aliases(app_handle: AppHandle) -> CommandResult<Vec<Alias>> {
    debug!("Listing all aliases");

    let aliases_dir = get_aliases_dir(&app_handle)?;
    let mut aliases = Vec::new();

    // Read all JSON files in aliases directory
    let entries = fs::read_dir(&aliases_dir)
        .map_err(|e| format!("Failed to read aliases directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read alias file: {}", e))?;

            let alias: Alias = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse alias: {}", e))?;

            aliases.push(alias);
        }
    }

    debug!("Found {} alias(es)", aliases.len());
    Ok(aliases)
}

/// Get a specific alias by ID
#[tauri::command]
pub async fn get_alias(id: String, app_handle: AppHandle) -> CommandResult<Alias> {
    debug!("Getting alias: {}", id);

    let aliases_dir = get_aliases_dir(&app_handle)?;
    let alias_file = aliases_dir.join(format!("{}.json", id));

    if !alias_file.exists() {
        return Err(format!("Alias not found: {}", id));
    }

    let content = fs::read_to_string(&alias_file)
        .map_err(|e| format!("Failed to read alias file: {}", e))?;

    let alias: Alias = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse alias: {}", e))?;

    Ok(alias)
}

/// Update an alias
#[tauri::command]
pub async fn update_alias(request: UpdateAliasRequest, app_handle: AppHandle) -> CommandResult<Alias> {
    info!("Updating alias: {}", request.id);

    // Get existing alias
    let mut alias = get_alias(request.id.clone(), app_handle.clone()).await?;

    // Update fields
    if let Some(name) = request.name {
        alias.name = name;
    }

    if let Some(pattern) = request.pattern {
        // Validate pattern before updating
        Alias::validate_pattern(&pattern)
            .map_err(|e| format!("Invalid pattern: {}", e))?;
        alias.pattern = pattern;
    }

    if let Some(action) = request.action {
        alias.action = match action.to_lowercase().as_str() {
            "send_command" => {
                let cmd = request.command.ok_or("command field required")?;
                AliasAction::SendCommand(cmd)
            }
            "send_commands" => {
                let cmds = request.commands.ok_or("commands field required")?;
                AliasAction::SendCommands(cmds)
            }
            "execute_script" => {
                let script = request.script.ok_or("script field required")?;
                AliasAction::ExecuteScript(script)
            }
            _ => return Err("Invalid action type".to_string()),
        };
    }

    if let Some(enabled) = request.enabled {
        alias.enabled = enabled;
    }

    // Get aliases directory
    let aliases_dir = get_aliases_dir(&app_handle)?;

    // Save alias to file
    let alias_file = aliases_dir.join(format!("{}.json", alias.id));
    let json = serde_json::to_string_pretty(&alias)
        .map_err(|e| format!("Failed to serialize alias: {}", e))?;

    fs::write(&alias_file, json)
        .map_err(|e| format!("Failed to write alias file: {}", e))?;

    info!("Alias '{}' updated in file", alias.name);

    // Sync with active session if connected
    let state: tauri::State<AppState> = app_handle.state();
    let mut session_guard = state.session.lock().await;
    if let Some(session) = session_guard.as_mut() {
        session.update_alias(alias.clone())
            .map_err(|e| format!("Failed to update alias in session: {}", e))?;
        info!("Alias updated in active session");
    }

    Ok(alias)
}

/// Delete an alias
#[tauri::command]
pub async fn delete_alias(id: String, app_handle: AppHandle) -> CommandResult<()> {
    info!("Deleting alias: {}", id);

    let alias_id = Uuid::parse_str(&id)
        .map_err(|e| format!("Invalid alias ID: {}", e))?;

    let aliases_dir = get_aliases_dir(&app_handle)?;
    let alias_file = aliases_dir.join(format!("{}.json", id));

    fs::remove_file(&alias_file)
        .map_err(|e| format!("Failed to delete alias: {}", e))?;

    info!("Alias {} deleted from file", id);

    // Remove from active session if connected
    let state: tauri::State<AppState> = app_handle.state();
    let mut session_guard = state.session.lock().await;
    if let Some(session) = session_guard.as_mut() {
        session.remove_alias(alias_id)
            .map_err(|e| format!("Failed to remove alias from session: {}", e))?;
        info!("Alias removed from active session");
    }

    Ok(())
}

// ============================================================================
// Trigger Management Commands
// ============================================================================

/// Trigger creation request
#[derive(Debug, Deserialize)]
pub struct CreateTriggerRequest {
    pub name: String,
    pub pattern: String,
    pub action: String, // "send_command", "display_text", "execute_script"
    pub command: Option<String>,
    pub text: Option<String>,
    pub script: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Trigger update request
#[derive(Debug, Deserialize)]
pub struct UpdateTriggerRequest {
    pub id: String,
    pub name: Option<String>,
    pub pattern: Option<String>,
    pub action: Option<String>,
    pub command: Option<String>,
    pub text: Option<String>,
    pub script: Option<String>,
    pub enabled: Option<bool>,
}

/// Get triggers directory path
fn get_triggers_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    let triggers_dir = app_data_dir.join("triggers");
    fs::create_dir_all(&triggers_dir)
        .map_err(|e| format!("Failed to create triggers directory: {}", e))?;
    Ok(triggers_dir)
}

/// Create a new trigger
#[tauri::command]
pub async fn create_trigger(request: CreateTriggerRequest, app_handle: AppHandle) -> CommandResult<Trigger> {
    info!("Creating trigger: {}", request.name);

    // Parse action
    let action = match request.action.to_lowercase().as_str() {
        "send_command" => {
            let cmd = request.command.ok_or("command field required for send_command action")?;
            TriggerAction::SendCommand(cmd)
        }
        "display_text" => {
            let text = request.text.ok_or("text field required for display_text action")?;
            TriggerAction::DisplayText(text)
        }
        "execute_script" => {
            let script = request.script.ok_or("script field required for execute_script action")?;
            TriggerAction::ExecuteScript(script)
        }
        _ => return Err("Invalid action type".to_string()),
    };

    // Create trigger
    let mut trigger = Trigger::new(request.name, request.pattern, action)
        .map_err(|e| format!("Failed to create trigger: {}", e))?;

    trigger.enabled = request.enabled;

    // Get triggers directory
    let triggers_dir = get_triggers_dir(&app_handle)?;

    // Save trigger to file
    let trigger_file = triggers_dir.join(format!("{}.json", trigger.id));
    let json = serde_json::to_string_pretty(&trigger)
        .map_err(|e| format!("Failed to serialize trigger: {}", e))?;

    fs::write(&trigger_file, json)
        .map_err(|e| format!("Failed to write trigger file: {}", e))?;

    info!("Trigger '{}' created with ID {}", trigger.name, trigger.id);

    // Sync with active session if connected
    let state: tauri::State<AppState> = app_handle.state();
    let mut session_guard = state.session.lock().await;
    if let Some(session) = session_guard.as_mut() {
        session.add_trigger(trigger.clone())
            .map_err(|e| format!("Failed to add trigger to session: {}", e))?;
        info!("Trigger synced to active session");
    }

    Ok(trigger)
}

/// List all triggers
#[tauri::command]
pub async fn list_triggers(app_handle: AppHandle) -> CommandResult<Vec<Trigger>> {
    debug!("Listing all triggers");

    let triggers_dir = get_triggers_dir(&app_handle)?;
    let mut triggers = Vec::new();

    // Read all JSON files in triggers directory
    let entries = fs::read_dir(&triggers_dir)
        .map_err(|e| format!("Failed to read triggers directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read trigger file: {}", e))?;

            let trigger: Trigger = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse trigger: {}", e))?;

            triggers.push(trigger);
        }
    }

    debug!("Found {} trigger(s)", triggers.len());
    Ok(triggers)
}

/// Get a specific trigger by ID
#[tauri::command]
pub async fn get_trigger(id: String, app_handle: AppHandle) -> CommandResult<Trigger> {
    debug!("Getting trigger: {}", id);

    let triggers_dir = get_triggers_dir(&app_handle)?;
    let trigger_file = triggers_dir.join(format!("{}.json", id));

    if !trigger_file.exists() {
        return Err(format!("Trigger not found: {}", id));
    }

    let content = fs::read_to_string(&trigger_file)
        .map_err(|e| format!("Failed to read trigger file: {}", e))?;

    let trigger: Trigger = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse trigger: {}", e))?;

    Ok(trigger)
}

/// Update a trigger
#[tauri::command]
pub async fn update_trigger(request: UpdateTriggerRequest, app_handle: AppHandle) -> CommandResult<Trigger> {
    info!("Updating trigger: {}", request.id);

    // Get existing trigger
    let mut trigger = get_trigger(request.id.clone(), app_handle.clone()).await?;

    // Update fields
    if let Some(name) = request.name {
        trigger.name = name;
    }

    if let Some(pattern) = request.pattern {
        // Validate pattern before updating
        Trigger::validate_pattern(&pattern)
            .map_err(|e| format!("Invalid pattern: {}", e))?;
        trigger.pattern = pattern;
    }

    if let Some(action) = request.action {
        trigger.action = match action.to_lowercase().as_str() {
            "send_command" => {
                let cmd = request.command.ok_or("command field required")?;
                TriggerAction::SendCommand(cmd)
            }
            "display_text" => {
                let text = request.text.ok_or("text field required")?;
                TriggerAction::DisplayText(text)
            }
            "execute_script" => {
                let script = request.script.ok_or("script field required")?;
                TriggerAction::ExecuteScript(script)
            }
            _ => return Err("Invalid action type".to_string()),
        };
    }

    if let Some(enabled) = request.enabled {
        trigger.enabled = enabled;
    }

    // Get triggers directory
    let triggers_dir = get_triggers_dir(&app_handle)?;

    // Save trigger to file
    let trigger_file = triggers_dir.join(format!("{}.json", trigger.id));
    let json = serde_json::to_string_pretty(&trigger)
        .map_err(|e| format!("Failed to serialize trigger: {}", e))?;

    fs::write(&trigger_file, json)
        .map_err(|e| format!("Failed to write trigger file: {}", e))?;

    info!("Trigger '{}' updated in file", trigger.name);

    // Sync with active session if connected
    let state: tauri::State<AppState> = app_handle.state();
    let mut session_guard = state.session.lock().await;
    if let Some(session) = session_guard.as_mut() {
        session.update_trigger(trigger.clone())
            .map_err(|e| format!("Failed to update trigger in session: {}", e))?;
        info!("Trigger updated in active session");
    }

    Ok(trigger)
}

/// Delete a trigger
#[tauri::command]
pub async fn delete_trigger(id: String, app_handle: AppHandle) -> CommandResult<()> {
    info!("Deleting trigger: {}", id);

    let trigger_id = Uuid::parse_str(&id)
        .map_err(|e| format!("Invalid trigger ID: {}", e))?;

    let triggers_dir = get_triggers_dir(&app_handle)?;
    let trigger_file = triggers_dir.join(format!("{}.json", id));

    fs::remove_file(&trigger_file)
        .map_err(|e| format!("Failed to delete trigger: {}", e))?;

    info!("Trigger {} deleted from file", id);

    // Remove from active session if connected
    let state: tauri::State<AppState> = app_handle.state();
    let mut session_guard = state.session.lock().await;
    if let Some(session) = session_guard.as_mut() {
        session.remove_trigger(trigger_id)
            .map_err(|e| format!("Failed to remove trigger from session: {}", e))?;
        info!("Trigger removed from active session");
    }

    Ok(())
}

// ============================================================================
// Highlight Management Commands
// ============================================================================

/// Highlight structure (for text coloring/formatting)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Highlight {
    pub id: Uuid,
    pub name: String,
    pub pattern: String,
    pub color: String,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub variables: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Highlight creation request
#[derive(Debug, Deserialize)]
pub struct CreateHighlightRequest {
    pub name: String,
    pub pattern: String,
    pub color: String,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub variables: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Highlight update request
#[derive(Debug, Deserialize)]
pub struct UpdateHighlightRequest {
    pub id: String,
    pub name: Option<String>,
    pub pattern: Option<String>,
    pub color: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub variables: Option<Vec<String>>,
    pub enabled: Option<bool>,
}

/// Get highlights directory path
fn get_highlights_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    let highlights_dir = app_data_dir.join("highlights");
    fs::create_dir_all(&highlights_dir)
        .map_err(|e| format!("Failed to create highlights directory: {}", e))?;
    Ok(highlights_dir)
}

/// Create a new highlight
#[tauri::command]
pub async fn create_highlight(request: CreateHighlightRequest, app_handle: AppHandle) -> CommandResult<Highlight> {
    info!("Creating highlight: {}", request.name);

    // Validate pattern
    regex::Regex::new(&request.pattern)
        .map_err(|e| format!("Invalid regex pattern: {}", e))?;

    let highlight = Highlight {
        id: Uuid::new_v4(),
        name: request.name,
        pattern: request.pattern,
        color: request.color,
        bold: request.bold,
        italic: request.italic,
        underline: request.underline,
        variables: request.variables,
        enabled: request.enabled,
    };

    // Get highlights directory
    let highlights_dir = get_highlights_dir(&app_handle)?;

    // Save highlight to file
    let highlight_file = highlights_dir.join(format!("{}.json", highlight.id));
    let json = serde_json::to_string_pretty(&highlight)
        .map_err(|e| format!("Failed to serialize highlight: {}", e))?;

    fs::write(&highlight_file, json)
        .map_err(|e| format!("Failed to write highlight file: {}", e))?;

    info!("Highlight '{}' created with ID {}", highlight.name, highlight.id);

    // Sync with active session if connected
    let state: tauri::State<AppState> = app_handle.state();
    let mut session_guard = state.session.lock().await;
    if let Some(session) = session_guard.as_mut() {
        // Convert commands::Highlight to automation::Highlight
        let auto_highlight = AutoHighlight::new(
            highlight.name.clone(),
            highlight.pattern.clone(),
            highlight.color.clone(),
        ).map_err(|e| format!("Failed to create automation highlight: {}", e))?;

        // Set additional fields (AutoHighlight::new returns a mutable object we need to configure)
        let mut auto_highlight = auto_highlight;
        auto_highlight.bold = highlight.bold;
        auto_highlight.italic = highlight.italic;
        auto_highlight.underline = highlight.underline;
        auto_highlight.variables = highlight.variables.clone();
        auto_highlight.enabled = highlight.enabled;

        session.add_highlight(auto_highlight)
            .map_err(|e| format!("Failed to add highlight to session: {}", e))?;
        info!("Highlight synced to active session");
    }

    Ok(highlight)
}

/// List all highlights
#[tauri::command]
pub async fn list_highlights(app_handle: AppHandle) -> CommandResult<Vec<Highlight>> {
    debug!("Listing all highlights");

    let highlights_dir = get_highlights_dir(&app_handle)?;
    let mut highlights = Vec::new();

    // Read all JSON files in highlights directory
    let entries = fs::read_dir(&highlights_dir)
        .map_err(|e| format!("Failed to read highlights directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read highlight file: {}", e))?;

            let highlight: Highlight = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse highlight: {}", e))?;

            highlights.push(highlight);
        }
    }

    debug!("Found {} highlight(s)", highlights.len());
    Ok(highlights)
}

/// Get a specific highlight by ID
#[tauri::command]
pub async fn get_highlight(id: String, app_handle: AppHandle) -> CommandResult<Highlight> {
    debug!("Getting highlight: {}", id);

    let highlights_dir = get_highlights_dir(&app_handle)?;
    let highlight_file = highlights_dir.join(format!("{}.json", id));

    if !highlight_file.exists() {
        return Err(format!("Highlight not found: {}", id));
    }

    let content = fs::read_to_string(&highlight_file)
        .map_err(|e| format!("Failed to read highlight file: {}", e))?;

    let highlight: Highlight = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse highlight: {}", e))?;

    Ok(highlight)
}

/// Update a highlight
#[tauri::command]
pub async fn update_highlight(request: UpdateHighlightRequest, app_handle: AppHandle) -> CommandResult<Highlight> {
    info!("Updating highlight: {}", request.id);

    // Get existing highlight
    let mut highlight = get_highlight(request.id.clone(), app_handle.clone()).await?;

    // Update fields
    if let Some(name) = request.name {
        highlight.name = name;
    }

    if let Some(pattern) = request.pattern {
        // Validate pattern before updating
        regex::Regex::new(&pattern)
            .map_err(|e| format!("Invalid regex pattern: {}", e))?;
        highlight.pattern = pattern;
    }

    if let Some(color) = request.color {
        highlight.color = color;
    }

    if let Some(bold) = request.bold {
        highlight.bold = bold;
    }

    if let Some(italic) = request.italic {
        highlight.italic = italic;
    }

    if let Some(underline) = request.underline {
        highlight.underline = underline;
    }

    if let Some(variables) = request.variables {
        highlight.variables = variables;
    }

    if let Some(enabled) = request.enabled {
        highlight.enabled = enabled;
    }

    // Get highlights directory
    let highlights_dir = get_highlights_dir(&app_handle)?;

    // Save highlight to file
    let highlight_file = highlights_dir.join(format!("{}.json", highlight.id));
    let json = serde_json::to_string_pretty(&highlight)
        .map_err(|e| format!("Failed to serialize highlight: {}", e))?;

    fs::write(&highlight_file, json)
        .map_err(|e| format!("Failed to write highlight file: {}", e))?;

    info!("Highlight '{}' updated in file", highlight.name);

    // Sync with active session if connected
    let state: tauri::State<AppState> = app_handle.state();
    let mut session_guard = state.session.lock().await;
    if let Some(session) = session_guard.as_mut() {
        // Convert commands::Highlight to automation::Highlight
        let auto_highlight = AutoHighlight::new(
            highlight.name.clone(),
            highlight.pattern.clone(),
            highlight.color.clone(),
        ).map_err(|e| format!("Failed to create automation highlight: {}", e))?;

        let mut auto_highlight = auto_highlight;
        auto_highlight.bold = highlight.bold;
        auto_highlight.italic = highlight.italic;
        auto_highlight.underline = highlight.underline;
        auto_highlight.variables = highlight.variables.clone();
        auto_highlight.enabled = highlight.enabled;

        session.update_highlight(auto_highlight)
            .map_err(|e| format!("Failed to update highlight in session: {}", e))?;
        info!("Highlight updated in active session");
    }

    Ok(highlight)
}

/// Delete a highlight
#[tauri::command]
pub async fn delete_highlight(id: String, app_handle: AppHandle) -> CommandResult<()> {
    info!("Deleting highlight: {}", id);

    let highlight_id = Uuid::parse_str(&id)
        .map_err(|e| format!("Invalid highlight ID: {}", e))?;

    let highlights_dir = get_highlights_dir(&app_handle)?;
    let highlight_file = highlights_dir.join(format!("{}.json", id));

    fs::remove_file(&highlight_file)
        .map_err(|e| format!("Failed to delete highlight: {}", e))?;

    info!("Highlight {} deleted from file", id);

    // Remove from active session if connected
    let state: tauri::State<AppState> = app_handle.state();
    let mut session_guard = state.session.lock().await;
    if let Some(session) = session_guard.as_mut() {
        session.remove_highlight(highlight_id)
            .map_err(|e| format!("Failed to remove highlight from session: {}", e))?;
        info!("Highlight removed from active session");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // TODO: These tests need to be rewritten as Tauri integration tests
    // using tauri::test::mock_builder() or similar test framework
    // The issue is that tauri::State cannot be easily mocked in unit tests
    //
    // For now, the command logic is tested indirectly through:
    // - Integration tests in tests/integration_tests.rs
    // - Manual testing through the Tauri frontend
    //
    // Future: Set up proper Tauri test harness for command testing
}
