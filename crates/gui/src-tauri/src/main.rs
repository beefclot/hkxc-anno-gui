// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cmd;
mod logger;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(feature = "tracing")]
            {
                Ok(crate::logger::init(app)?)
            }
            #[cfg(not(feature = "tracing"))]
            {
                let _ = app;
                Ok(())
            }
        })
        .invoke_handler(tauri::generate_handler![
            crate::cmd::dump_annotations,
            crate::cmd::update_annotations,
            crate::logger::change_log_level,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
