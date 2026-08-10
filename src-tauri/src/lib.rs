// RustyMeals Tauri core — library entry point.
pub mod db;
pub mod state;
pub mod api;

use state::AppState;
use tauri::Manager;

/// Grouping commands inside a module breaks the macro expansion collision.
pub mod commands {
    use super::AppState;
    use crate::db::{self, RecipeSummary};
    use crate::api::MealieClient;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tauri::command]
    pub async fn get_recipes(
        state: tauri::State<'_, AppState>,
        query: Option<String>,
    ) -> Result<Vec<RecipeSummary>, String> {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::get_recipe_summaries(&conn, query.as_deref()).map_err(|e| e.to_string())
    }

    #[tauri::command]
    pub async fn get_recipe_detail(
        state: tauri::State<'_, AppState>,
        id: String,
    ) -> Result<Option<String>, String> {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::get_recipe_raw_json(&conn, &id).map_err(|e| e.to_string())
    }

    #[tauri::command]
    pub async fn toggle_offline_recipe(
        state: tauri::State<'_, AppState>,
        id: String,
        offline: bool,
    ) -> Result<(), String> {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::set_offline_available(&conn, &id, offline, None).map_err(|e| e.to_string())
    }

    #[tauri::command]
    pub async fn trigger_sync(
        state: tauri::State<'_, AppState>,
        base_url: String,
        token: String,
    ) -> Result<(), String> {
        let client = MealieClient::new(base_url, token);
        let online_recipes = client.fetch_all_recipes().await?;

        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        for recipe in online_recipes {
            let tags = recipe.tags.unwrap_or_default();
            db::upsert_recipe_summary(
                &conn,
                &recipe.id,
                &recipe.name,
                recipe.description.as_deref(),
                None,
                "{}",
                &tags,
                timestamp,
            ).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let app_dir = app.path().app_data_dir().expect("Failed to resolve app data dir");
            std::fs::create_dir_all(&app_dir).expect("Failed to create app data dir");
            let db_path = app_dir.join("rustymealie.db");
            let conn = db::init_db(&db_path).expect("Failed to initialize SQLite database");
            app.manage(AppState::new(conn));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_recipes,
            commands::get_recipe_detail,
            commands::toggle_offline_recipe,
            commands::trigger_sync
        ])
        .run(tauri::generate_context!())
        .expect("error while running RustyMeals");
}
