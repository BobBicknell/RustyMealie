use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

/// Server connection details. Held in memory only, after the frontend
/// supplies them once via `set_credentials` (on startup, once settings load
/// from `tauri-plugin-store`, and whenever the user saves new ones). Mutating
/// commands read from here instead of taking `base_url`/`token` as
/// parameters on every call, so the token isn't re-threaded through the
/// invoke bridge for each shopping-list tap.
#[derive(Clone, Default)]
pub struct Credentials {
    pub base_url: String,
    pub token: String,
}

/// The central state managed by Tauri and accessed by frontend commands.
pub struct AppState {
    pub db: Mutex<Connection>,
    pub images_dir: PathBuf,
    pub credentials: Mutex<Option<Credentials>>,
}

impl AppState {
    pub fn new(conn: Connection, images_dir: PathBuf) -> Self {
        Self {
            db: Mutex::new(conn),
            images_dir,
            credentials: Mutex::new(None),
        }
    }
}
