mod commands;
mod fs_ops;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::read_dir,
            commands::get_home_dir,
            commands::get_parent_dir,
            commands::open_entry,
            commands::rename_entry,
            commands::delete_entry,
            commands::copy_entry,
            commands::move_entry,
            commands::calculate_dir_size,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
