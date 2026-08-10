use std::sync::Mutex;
use rusqlite::Connection;

/// The central state managed by Tauri and accessed by frontend commands.
pub struct AppState {
    pub db: Mutex<Connection>,
}

impl AppState {
    pub fn new(conn: Connection) -> Self {
        Self {
            db: Mutex::new(conn),
        }
    }
}
