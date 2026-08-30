// RustyMeals Tauri core — library entry point.
pub mod api;
pub mod db;
pub mod state;

use state::AppState;
use tauri::Manager;

/// Grouping commands inside a module breaks the macro expansion collision.
pub mod commands {
    use super::AppState;
    use crate::api::MealieClient;
    use crate::db::{self, ImageToFetch, OfflineRecipeRef, RecipeSummary};
    use crate::state::Credentials;
    use serde::Serialize;
    use std::path::Path;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tauri::Emitter;

    fn unix_now() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    #[tauri::command]
    pub fn get_app_version() -> Result<String, String> {
        Ok(env!("CARGO_PKG_VERSION").to_string())
    }

    /// Store the server URL/token in memory for this session. Called once
    /// after settings load from the store on startup, and again whenever the
    /// user saves new ones from the Settings screen. Every other command
    /// that talks to the server reads from here via `require_credentials`
    /// instead of taking the token as an argument.
    #[tauri::command]
    pub fn set_credentials(
        state: tauri::State<'_, AppState>,
        base_url: String,
        token: String,
    ) -> Result<(), String> {
        let mut creds = state.credentials.lock().map_err(|e| e.to_string())?;
        *creds = Some(Credentials { base_url, token });
        Ok(())
    }

    /// Fetch the in-memory credentials, or a clear error if the frontend
    /// hasn't called `set_credentials` yet this session (e.g. settings were
    /// never configured).
    fn require_credentials(state: &tauri::State<'_, AppState>) -> Result<Credentials, String> {
        state
            .credentials
            .lock()
            .map_err(|e| e.to_string())?
            .clone()
            .ok_or_else(|| "Not connected — set the server URL and token in Settings".to_string())
    }

    #[tauri::command]
    pub async fn get_recipes(
        state: tauri::State<'_, AppState>,
        query: Option<String>,
        category: Option<String>,
        tag: Option<String>,
    ) -> Result<Vec<RecipeSummary>, String> {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::get_recipe_summaries(&conn, query.as_deref(), category.as_deref(), tag.as_deref())
            .map_err(|e| e.to_string())
    }

    #[tauri::command]
    pub async fn get_recipe_detail(
        state: tauri::State<'_, AppState>,
        id: String,
    ) -> Result<Option<String>, String> {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::get_recipe_raw_json(&conn, &id).map_err(|e| e.to_string())
    }

    /// Fetch a recipe's full payload from the server and cache it locally
    /// so subsequent opens work offline. Used when the locally-stored
    /// payload is still the `"{}"` placeholder from the metadata-only sync.
    #[tauri::command]
    pub async fn fetch_recipe_detail(
        state: tauri::State<'_, AppState>,
        id: String,
        slug: String,
    ) -> Result<String, String> {
        let creds = require_credentials(&state)?;
        let client = MealieClient::new(creds.base_url, creds.token);
        let detail = client.fetch_recipe_detail(&slug).await?;
        let raw_json = detail.to_string();
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::update_recipe_payload(&conn, &id, &raw_json, None, unix_now())
            .map_err(|e| e.to_string())?;
        Ok(raw_json)
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

    #[derive(Debug, Serialize)]
    pub struct SyncReport {
        pub total_recipes: usize,
        pub details_synced: usize,
        pub images_downloaded: usize,
        pub errors: usize,
        pub finished_at: i64,
    }

    /// Live progress emitted over the `sync-progress` event while a sync
    /// is in flight, so the UI can show what is happening instead of an
    /// indefinite spinner.
    #[derive(Debug, Serialize, Clone)]
    pub struct SyncProgress {
        pub phase: String,
        pub processed: usize,
        pub total: usize,
        pub message: String,
    }

    fn emit_progress(
        app: &tauri::AppHandle,
        phase: &str,
        processed: usize,
        total: usize,
        message: &str,
    ) {
        let _ = app.emit(
            "sync-progress",
            SyncProgress {
                phase: phase.to_string(),
                processed,
                total,
                message: message.to_string(),
            },
        );
    }

    fn write_image_file(
        images_dir: &Path,
        recipe_id: &str,
        file_name: &str,
        bytes: &[u8],
    ) -> Result<String, ()> {
        let ext = file_name
            .rfind('.')
            .map(|i| &file_name[i + 1..])
            .filter(|e| !e.is_empty())
            .unwrap_or("webp");
        let path = images_dir.join(format!("{recipe_id}.{ext}"));
        std::fs::write(&path, bytes).map_err(|_| ())?;
        Ok(path.to_string_lossy().into_owned())
    }

    #[tauri::command]
    pub async fn trigger_sync(
        app: tauri::AppHandle,
        state: tauri::State<'_, AppState>,
    ) -> Result<SyncReport, String> {
        let creds = require_credentials(&state)?;
        let base_url = creds.base_url;
        emit_progress(&app, "connect", 0, 0, "Connecting to server…");

        let client = MealieClient::new(base_url.clone(), creds.token);
        let summaries = client.fetch_all_recipe_summaries().await?;

        let timestamp = unix_now();
        let total_recipes = summaries.len();

        // One transaction for the whole metadata pass instead of a lock
        // acquisition + implicit transaction per recipe — for a few hundred
        // recipes that's the difference between one commit and hundreds.
        {
            let mut conn = state.db.lock().map_err(|e| e.to_string())?;
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            for (index, summary) in summaries.iter().enumerate() {
                let (Some(id), Some(name)) = (&summary.id, &summary.name) else {
                    continue;
                };
                db::upsert_recipe_summary(
                    &tx,
                    id,
                    name,
                    summary.slug.as_deref(),
                    summary.description.as_deref(),
                    summary.image.as_deref(),
                    "{}",
                    &summary.tag_names(),
                    &summary.category_names(),
                    timestamp,
                )
                .map_err(|e| e.to_string())?;
                // Emit roughly every 10 recipes (plus the first/last) so the
                // progress bar visibly moves through a large sync instead of
                // sitting frozen until the very end.
                if index == 0 || index + 1 == total_recipes || index % 10 == 0 {
                    emit_progress(
                        &app,
                        "recipes",
                        index + 1,
                        total_recipes,
                        &format!("Indexing recipes… {} of {total_recipes}", index + 1),
                    );
                }
            }
            tx.commit().map_err(|e| e.to_string())?;
        }
        emit_progress(
            &app,
            "recipes",
            total_recipes,
            total_recipes,
            "Recipe list ready",
        );

        // Download a local thumbnail for every recipe that is missing one,
        // so the list and detail views show images even offline. Recipes
        // whose download fails retry on the next sync. Downloads run in
        // parallel batches so a slow connection does not serialize each
        // thumbnail fetch.
        let images_to_fetch = {
            let conn = state.db.lock().map_err(|e| e.to_string())?;
            db::get_recipes_needing_image(&conn).map_err(|e| e.to_string())?
        };

        let mut images_downloaded = 0usize;
        let images_total = images_to_fetch.len();
        let mut processed_images = 0usize;
        let mut batch = Vec::new();

        for ImageToFetch { id, .. } in images_to_fetch {
            let worker = client.clone();
            let task_id = id.clone();
            batch.push(tokio::spawn(async move {
                // Images are a nice-to-have cached offline copy; a slow or
                // hung fetch must not wedge the whole sync. Cap each recipe's
                // image attempt so a bad connection drains the batch quickly.
                let result = tokio::time::timeout(
                    Duration::from_secs(15),
                    worker.download_recipe_image(&id, "min-original.webp"),
                )
                .await
                .unwrap_or(Err("image download timed out".to_string()));
                (task_id, result)
            }));
            if batch.len() >= 4 {
                for handle in batch.drain(..) {
                    processed_images += 1;
                    if let Ok((id, Ok(bytes))) = handle.await {
                        if let Ok(path) =
                            write_image_file(&state.images_dir, &id, "min-original.webp", &bytes)
                        {
                            if let Ok(conn) = state.db.lock() {
                                if db::set_recipe_image(&conn, &id, &path).is_ok() {
                                    images_downloaded += 1;
                                }
                            }
                        }
                    }
                    emit_progress(
                        &app,
                        "images",
                        processed_images,
                        images_total,
                        &format!("Downloading thumbnail {processed_images} of {images_total}…"),
                    );
                }
            }
        }
        for handle in batch {
            processed_images += 1;
            if let Ok((id, Ok(bytes))) = handle.await {
                if let Ok(path) =
                    write_image_file(&state.images_dir, &id, "min-original.webp", &bytes)
                {
                    if let Ok(conn) = state.db.lock() {
                        if db::set_recipe_image(&conn, &id, &path).is_ok() {
                            images_downloaded += 1;
                        }
                    }
                }
            }
            emit_progress(
                &app,
                "images",
                processed_images,
                images_total,
                &format!("Downloading thumbnail {processed_images} of {images_total}…"),
            );
        }
        if images_total > 0 {
            emit_progress(
                &app,
                "images",
                images_total,
                images_total,
                "Thumbnails ready",
            );
        }

        let offline_refs = {
            let conn = state.db.lock().map_err(|e| e.to_string())?;
            db::get_offline_recipe_refs(&conn).map_err(|e| e.to_string())?
        };

        let mut details_synced = 0usize;
        let mut errors = 0usize;
        let details_total = offline_refs.len();

        for (index, OfflineRecipeRef { id, slug, .. }) in offline_refs.into_iter().enumerate() {
            let Some(slug) = slug else {
                errors += 1;
                continue;
            };
            emit_progress(
                &app,
                "details",
                index,
                details_total,
                &format!("Pulling offline recipe {index} of {details_total}…"),
            );

            let detail = match client.fetch_recipe_detail(&slug).await {
                Ok(detail) => detail,
                Err(_) => {
                    errors += 1;
                    continue;
                }
            };

            // The thumbnail for offline recipes is downloaded by the loop
            // above unless a copy is already cached; only download it here
            // if it is still missing.
            let needs_image = {
                let conn = state.db.lock().map_err(|e| e.to_string())?;
                db::get_recipe_image_path(&conn, &id)
                    .map_err(|e| e.to_string())?
                    .is_none()
            };

            let local_image_path = if needs_image {
                match client.download_recipe_image(&id, "min-original.webp").await {
                    Ok(bytes) => {
                        write_image_file(&state.images_dir, &id, "min-original.webp", &bytes).ok()
                    }
                    Err(_) => None,
                }
            } else {
                None
            };

            let raw_json = detail.to_string();
            let conn = state.db.lock().map_err(|e| e.to_string())?;
            if db::update_recipe_payload(
                &conn,
                &id,
                &raw_json,
                local_image_path.as_deref(),
                timestamp,
            )
            .is_ok()
            {
                details_synced += 1;
            } else {
                errors += 1;
            }
        }

        {
            let conn = state.db.lock().map_err(|e| e.to_string())?;
            db::set_sync_meta(&conn, "last_sync_at", &timestamp.to_string())
                .map_err(|e| e.to_string())?;
            db::set_sync_meta(&conn, "last_sync_count", &total_recipes.to_string())
                .map_err(|e| e.to_string())?;
            db::set_sync_meta(&conn, "server_url", &base_url).map_err(|e| e.to_string())?;
        }

        emit_progress(&app, "done", 0, 0, "Sync complete");

        Ok(SyncReport {
            total_recipes,
            details_synced,
            images_downloaded,
            errors,
            finished_at: unix_now(),
        })
    }

    #[derive(Debug, Serialize)]
    pub struct SyncStatus {
        pub last_sync_at: Option<i64>,
        pub last_sync_count: Option<i64>,
        pub server_url: Option<String>,
    }

    #[tauri::command]
    pub async fn get_sync_status(state: tauri::State<'_, AppState>) -> Result<SyncStatus, String> {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let meta = |key: &str| db::get_sync_meta(&conn, key).map_err(|e| e.to_string());
        let as_i64 = |value: Option<String>| value.and_then(|raw| raw.parse::<i64>().ok());

        Ok(SyncStatus {
            last_sync_at: as_i64(meta("last_sync_at")?),
            last_sync_count: as_i64(meta("last_sync_count")?),
            server_url: meta("server_url")?,
        })
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct ShoppingListItem {
        pub id: String,
        pub display: String,
        pub note: Option<String>,
        #[serde(default)]
        pub checked: bool,
        #[serde(default)]
        pub position: i64,
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct ShoppingList {
        pub id: String,
        pub name: String,
        pub items: Vec<ShoppingListItem>,
    }

    /// Build a UI-facing shopping list from a cached raw payload without
    /// letting a malformed row take the whole screen down.
    fn parse_local_shopping_list(id: &str, name: &str, raw_json: &str) -> ShoppingList {
        let detail = serde_json::from_str::<crate::api::MealieShoppingList>(raw_json).unwrap_or(
            crate::api::MealieShoppingList {
                id: Some(id.to_string()),
                name: Some(name.to_string()),
                list_items: Vec::new(),
            },
        );

        let mut items: Vec<ShoppingListItem> = detail
            .list_items
            .into_iter()
            .filter_map(|item| {
                Some(ShoppingListItem {
                    id: item.id?,
                    display: item.display.unwrap_or_default(),
                    note: item.note,
                    checked: item.checked,
                    position: item.position,
                })
            })
            .collect();
        items.sort_by(|a, b| {
            a.position
                .cmp(&b.position)
                .then_with(|| a.display.cmp(&b.display))
        });
        ShoppingList {
            id: id.to_string(),
            name: name.to_string(),
            items,
        }
    }

    /// Read every cached shopping list (with items) from the local database.
    #[tauri::command]
    pub async fn get_shopping_lists(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<ShoppingList>, String> {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let rows = db::get_shopping_lists_raw(&conn).map_err(|e| e.to_string())?;
        Ok(rows
            .into_iter()
            .map(|(id, name, raw)| parse_local_shopping_list(&id, &name, &raw))
            .collect())
    }

    #[derive(Debug, Serialize)]
    pub struct ShoppingListsSyncReport {
        pub lists: usize,
        pub items: usize,
        pub errors: usize,
    }

    /// Pull the household's shopping lists and their items from the server
    /// and refresh the local cache. Lists deleted remotely are pruned.
    #[tauri::command]
    pub async fn refresh_shopping_lists(
        state: tauri::State<'_, AppState>,
    ) -> Result<ShoppingListsSyncReport, String> {
        let creds = require_credentials(&state)?;
        let client = MealieClient::new(creds.base_url, creds.token);
        let summaries = client.fetch_shopping_lists().await?;
        let timestamp = unix_now();

        let mut items = 0usize;
        let mut errors = 0usize;
        for summary in &summaries {
            let id = summary.id.clone().unwrap_or_default();
            let name = summary
                .name
                .clone()
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(|| id.clone());
            if id.is_empty() {
                errors += 1;
                continue;
            }
            let detail = match client.fetch_shopping_list(&id).await {
                Ok(detail) => detail,
                Err(_) => {
                    errors += 1;
                    continue;
                }
            };
            items += detail.list_items.len();
            let raw = serde_json::to_string(&detail).unwrap_or_else(|_| "{}".into());
            let conn = state.db.lock().map_err(|e| e.to_string())?;
            db::upsert_shopping_list(&conn, &id, &name, &raw, timestamp)
                .map_err(|e| e.to_string())?;
        }

        let keep_ids: Vec<String> = summaries.iter().filter_map(|s| s.id.clone()).collect();
        {
            let conn = state.db.lock().map_err(|e| e.to_string())?;
            db::delete_shopping_lists_except(&conn, &keep_ids).map_err(|e| e.to_string())?;
        }

        Ok(ShoppingListsSyncReport {
            lists: summaries.len(),
            items,
            errors,
        })
    }

    /// Toggle an item's `checked` flag on the server, then refresh that
    /// list's local cache from the authoritative response.
    #[tauri::command]
    pub async fn toggle_shopping_item(
        state: tauri::State<'_, AppState>,
        list_id: String,
        item_id: String,
        checked: bool,
    ) -> Result<ShoppingList, String> {
        let creds = require_credentials(&state)?;
        let client = MealieClient::new(creds.base_url, creds.token);
        client.set_shopping_item_checked(&item_id, checked).await?;

        let detail = client.fetch_shopping_list(&list_id).await?;
        let name = detail
            .name
            .clone()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| list_id.clone());
        let raw = serde_json::to_string(&detail).unwrap_or_else(|_| "{}".into());

        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::upsert_shopping_list(&conn, &list_id, &name, &raw, unix_now())
            .map_err(|e| e.to_string())?;

        Ok(parse_local_shopping_list(&list_id, &name, &raw))
    }

    /// Add an item to a shopping list on the server, then refresh that
    /// list's local cache.
    #[tauri::command]
    pub async fn add_shopping_list_item(
        state: tauri::State<'_, AppState>,
        list_id: String,
        note: String,
    ) -> Result<ShoppingList, String> {
        let creds = require_credentials(&state)?;
        let client = MealieClient::new(creds.base_url, creds.token);
        client.create_shopping_list_item(&list_id, &note).await?;

        let detail = client.fetch_shopping_list(&list_id).await?;
        let name = detail
            .name
            .clone()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| list_id.clone());
        let raw = serde_json::to_string(&detail).unwrap_or_else(|_| "{}".into());

        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::upsert_shopping_list(&conn, &list_id, &name, &raw, unix_now())
            .map_err(|e| e.to_string())?;

        Ok(parse_local_shopping_list(&list_id, &name, &raw))
    }

    /// Add all of a recipe's ingredients to a shopping list on the server,
    /// then refresh that list's local cache. `recipe_id` is the recipe's
    /// UUID (the summary `id`, not its slug).
    #[tauri::command]
    pub async fn add_recipe_to_shopping_list(
        state: tauri::State<'_, AppState>,
        list_id: String,
        recipe_id: String,
    ) -> Result<ShoppingList, String> {
        let creds = require_credentials(&state)?;
        let client = MealieClient::new(creds.base_url, creds.token);
        let detail = client
            .add_recipe_to_shopping_list(&list_id, &recipe_id)
            .await?;

        let name = detail
            .name
            .clone()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| list_id.clone());
        let raw = serde_json::to_string(&detail).unwrap_or_else(|_| "{}".into());

        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::upsert_shopping_list(&conn, &list_id, &name, &raw, unix_now())
            .map_err(|e| e.to_string())?;

        Ok(parse_local_shopping_list(&list_id, &name, &raw))
    }

    /// Permanently delete every item on a shopping list whose `checked` flag
    /// is set, then refresh that list's local cache.
    #[tauri::command]
    pub async fn clear_checked_shopping_items(
        state: tauri::State<'_, AppState>,
        list_id: String,
    ) -> Result<ShoppingList, String> {
        let creds = require_credentials(&state)?;
        let client = MealieClient::new(creds.base_url, creds.token);
        let detail = client.fetch_shopping_list(&list_id).await?;
        let checked_ids: Vec<String> = detail
            .list_items
            .iter()
            .filter(|item| item.checked)
            .filter_map(|item| item.id.clone())
            .collect();

        client.delete_shopping_items(&checked_ids).await?;

        let detail = client.fetch_shopping_list(&list_id).await?;
        let name = detail
            .name
            .clone()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| list_id.clone());
        let raw = serde_json::to_string(&detail).unwrap_or_else(|_| "{}".into());

        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::upsert_shopping_list(&conn, &list_id, &name, &raw, unix_now())
            .map_err(|e| e.to_string())?;

        Ok(parse_local_shopping_list(&list_id, &name, &raw))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to resolve app data dir");
            std::fs::create_dir_all(&app_dir).expect("Failed to create app data dir");
            let images_dir = app_dir.join("images");
            std::fs::create_dir_all(&images_dir).expect("Failed to create image cache dir");
            let db_path = app_dir.join("rustymealie.db");
            let conn = db::init_db(&db_path).expect("Failed to initialize SQLite database");
            app.manage(AppState::new(conn, images_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_version,
            commands::set_credentials,
            commands::get_recipes,
            commands::get_recipe_detail,
            commands::fetch_recipe_detail,
            commands::toggle_offline_recipe,
            commands::trigger_sync,
            commands::get_sync_status,
            commands::get_shopping_lists,
            commands::refresh_shopping_lists,
            commands::toggle_shopping_item,
            commands::add_shopping_list_item,
            commands::add_recipe_to_shopping_list,
            commands::clear_checked_shopping_items
        ])
        .run(tauri::generate_context!())
        .expect("error while running RustyMeals");
}
