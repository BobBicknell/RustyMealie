// RustyMeals Tauri core — library entry point.
//
// The mobile entry point (`run`) is annotated with `tauri::mobile_entry_point`
// so the same code path is used on Android/iOS and desktop.
use tauri::Manager;
mod db;
mod state;

use state::AppState;
use db::RecipeSummary;


/// Tauri command to retrieve list/search views.
#[tauri::command]
async fn get_recipes(
    state: tauri::State<'_, AppState>,
    query: Option<String>,
) -> Result<Vec<RecipeSummary>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::get_recipe_summaries(&conn, query.as_deref()).map_err(|e| e.to_string())
}

/// Tauri command to fetch a single recipe details raw payload.
#[tauri::command]
async fn get_recipe_detail(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<Option<String>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::get_recipe_raw_json(&conn, &id).map_err(|e| e.to_string())
}

/// Tauri command to mark a recipe offline.
#[tauri::command]
async fn toggle_offline_recipe(
    state: tauri::State<'_, AppState>,
    id: String,
    offline: bool,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::set_offline_available(&conn, &id, offline, None).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            // Locate a persistent path for the mobile/desktop app data
            let app_dir = app.path()
                .app_data_dir()
                .expect("Failed to resolve app data directory");

            // Create directory path if it does not exist yet
            std::fs::create_dir_all(&app_dir).expect("Failed to create app data directory");

            let db_path = app_dir.join("rustymealie.db");

            // Initialize database connection & run migrations
            let conn = db::init_db(&db_path).expect("Failed to initialize SQLite database");

            // Register global state with Tauri
            app.manage(AppState::new(conn));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_recipes,
            get_recipe_detail,
            toggle_offline_recipe
        ])
        .run(tauri::generate_context!())
        .expect("error while running RustyMeals");
}
