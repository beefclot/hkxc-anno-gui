/// Initializes logger.
///
/// # Errors
/// Double init
#[cfg(feature = "tracing")]
pub(crate) fn init(app: &tauri::App) -> Result<(), tracing_rotation::error::Error> {
    let log_name = format!("{}.log", app.package_info().name);
    Ok(tracing_rotation::init("./logs", &log_name)?)
}

/// Change log level
///
/// # Errors
/// If logger uninitialized.
///
/// # Note
/// - If unknown log level. fallback to `error`.
/// - log_level: "trace" | "debug" | "info" | "warn" | "error" otherwise "error".
#[tauri::command]
pub(crate) fn change_log_level(level: &str) -> Result<(), String> {
    #[cfg(feature = "tracing")]
    {
        tracing_rotation::change_level(level).map_err(|e| e.to_string())
    }
    #[cfg(not(feature = "tracing"))]
    {
        let _ = level;
        Ok(())
    }
}
