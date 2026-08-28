use std::path::PathBuf;
use std::sync::Mutex;
use rusqlite::Connection;

/// The central state managed by Tauri and accessed by frontend commands.
pub struct AppState {
    pub db: Mutex<Connection>,
    pub images_dir: PathBuf,
}

impl AppState {
    pub fn new(conn: Connection, images_dir: PathBuf) -> Self {
        Self {
            db: Mutex::new(conn),
            images_dir,
        }
    }
}
