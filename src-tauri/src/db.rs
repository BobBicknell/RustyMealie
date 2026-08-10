//! SQLite persistence layer for RustyMeals.
//!
//! Owns connection setup, schema migrations, and CRUD helpers for the
//! three tables defined in the implementation plan: `recipes`,
//! `shopping_lists`, and `sync_meta`. Callers go through `AppState`
//! (see `state.rs`) rather than opening connections directly.

use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use serde::{Deserialize, Serialize};

/// A lightweight summary of a recipe, used for list/search views.
/// Built from the denormalized columns so we avoid parsing `raw_json`
/// just to render a list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub image_path: Option<String>,
    pub tags: Vec<String>,
    pub marked_offline: bool,
}

/// Open (or create) the SQLite database at `db_path` and run any
/// pending migrations. Safe to call every app start — migrations are
/// idempotent (`CREATE TABLE IF NOT EXISTS`).
pub fn init_db(db_path: &std::path::Path) -> SqlResult<Connection> {
    let conn = Connection::open(db_path)?;

    // Recommended pragmas for a mobile app: WAL improves concurrent
    // read/write behavior, and foreign_keys enforces referential
    // integrity if we add relations later.
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;

    run_migrations(&conn)?;
    Ok(conn)
}

fn run_migrations(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS recipes (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            image_path TEXT,
            raw_json TEXT NOT NULL,
            tags TEXT,
            marked_offline INTEGER NOT NULL DEFAULT 0,
            last_synced_at INTEGER
        );

        CREATE TABLE IF NOT EXISTS shopping_lists (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            raw_json TEXT NOT NULL,
            last_synced_at INTEGER
        );

        CREATE TABLE IF NOT EXISTS sync_meta (
            key TEXT PRIMARY KEY,
            value TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_recipes_name ON recipes(name);
        CREATE INDEX IF NOT EXISTS idx_recipes_marked_offline ON recipes(marked_offline);
        "#,
    )
}

/// Upsert a recipe's metadata (used during the metadata-list sync pull).
/// Does not touch `marked_offline` — that flag is only changed via
/// `set_offline_available`, so a routine metadata sync never silently
/// un-marks a recipe the user flagged for offline use.
pub fn upsert_recipe_summary(
    conn: &Connection,
    id: &str,
    name: &str,
    description: Option<&str>,
    image_path: Option<&str>,
    raw_json: &str,
    tags: &[String],
    synced_at: i64,
) -> SqlResult<()> {
    let tags_joined = tags.join(",");
    conn.execute(
        r#"
        INSERT INTO recipes (id, name, description, image_path, raw_json, tags, last_synced_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            description = excluded.description,
            image_path = COALESCE(excluded.image_path, recipes.image_path),
            raw_json = excluded.raw_json,
            tags = excluded.tags,
            last_synced_at = excluded.last_synced_at
        "#,
        params![id, name, description, image_path, raw_json, tags_joined, synced_at],
    )?;
    Ok(())
}

/// Fetch recipe summaries, optionally filtered by a case-insensitive
/// substring match against name or tags.
pub fn get_recipe_summaries(conn: &Connection, query: Option<&str>) -> SqlResult<Vec<RecipeSummary>> {
    let sql = match query {
        Some(_) => {
            "SELECT id, name, description, image_path, tags, marked_offline FROM recipes \
             WHERE name LIKE ?1 OR tags LIKE ?1 ORDER BY name"
        }
        None => "SELECT id, name, description, image_path, tags, marked_offline FROM recipes ORDER BY name",
    };

    let mut stmt = conn.prepare(sql)?;
    let map_row = |row: &rusqlite::Row| -> SqlResult<RecipeSummary> {
        let tags_raw: Option<String> = row.get(4)?;
        let tags = tags_raw
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        Ok(RecipeSummary {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            image_path: row.get(3)?,
            tags,
            marked_offline: row.get::<_, i64>(5)? != 0,
        })
    };

    let rows = match query {
        Some(q) => {
            let pattern = format!("%{q}%");
            stmt.query_map(params![pattern], map_row)?
                .collect::<SqlResult<Vec<_>>>()?
        }
        None => stmt.query_map([], map_row)?.collect::<SqlResult<Vec<_>>>()?,
    };

    Ok(rows)
}

/// Fetch a single recipe's full `raw_json` payload by id, if present.
pub fn get_recipe_raw_json(conn: &Connection, id: &str) -> SqlResult<Option<String>> {
    conn.query_row(
        "SELECT raw_json FROM recipes WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )
        .optional()
}

/// Mark (or unmark) a recipe as available offline, and optionally
/// record where its downloaded image lives on disk.
pub fn set_offline_available(
    conn: &Connection,
    id: &str,
    offline: bool,
    image_path: Option<&str>,
) -> SqlResult<()> {
    conn.execute(
        "UPDATE recipes SET marked_offline = ?1, image_path = COALESCE(?2, image_path) WHERE id = ?3",
        params![offline as i64, image_path, id],
    )?;
    Ok(())
}

/// Ids of every recipe currently flagged offline — used by the sync
/// engine to know which recipes need their full body + image re-pulled.
pub fn get_offline_recipe_ids(conn: &Connection) -> SqlResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id FROM recipes WHERE marked_offline = 1")?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<SqlResult<Vec<_>>>()?;
    Ok(rows)
}

/// Upsert a shopping list's raw payload.
pub fn upsert_shopping_list(conn: &Connection, id: &str, name: &str, raw_json: &str, synced_at: i64) -> SqlResult<()> {
    conn.execute(
        r#"
        INSERT INTO shopping_lists (id, name, raw_json, last_synced_at)
        VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            raw_json = excluded.raw_json,
            last_synced_at = excluded.last_synced_at
        "#,
        params![id, name, raw_json, synced_at],
    )?;
    Ok(())
}

/// Read-only view of a shopping list, deserialized by the caller.
pub fn get_shopping_lists_raw(conn: &Connection) -> SqlResult<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT id, raw_json FROM shopping_lists ORDER BY name")?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
        .collect::<SqlResult<Vec<_>>>()?;
    Ok(rows)
}

/// Get/set a single `sync_meta` key (e.g. `last_full_sync_at`).
pub fn get_sync_meta(conn: &Connection, key: &str) -> SqlResult<Option<String>> {
    conn.query_row("SELECT value FROM sync_meta WHERE key = ?1", params![key], |row| row.get(0))
        .optional()
}

pub fn set_sync_meta(conn: &Connection, key: &str, value: &str) -> SqlResult<()> {
    conn.execute(
        "INSERT INTO sync_meta (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn migration_creates_all_tables() {
        let conn = test_conn();
        for table in ["recipes", "shopping_lists", "sync_meta"] {
            let count: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    params![table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "expected table {table} to exist");
        }
    }

    #[test]
    fn upsert_and_fetch_recipe_summary() {
        let conn = test_conn();
        upsert_recipe_summary(
            &conn,
            "r1",
            "Pasta Carbonara",
            Some("Classic Roman pasta"),
            None,
            r#"{"id":"r1"}"#,
            &["italian".to_string(), "pasta".to_string()],
            1000,
        )
            .unwrap();

        let all = get_recipe_summaries(&conn, None).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "Pasta Carbonara");
        assert_eq!(all[0].tags, vec!["italian", "pasta"]);
        assert!(!all[0].marked_offline);
    }

    #[test]
    fn search_filters_by_name_and_tags() {
        let conn = test_conn();
        upsert_recipe_summary(&conn, "r1", "Pasta Carbonara", None, None, "{}", &["italian".into()], 1000).unwrap();
        upsert_recipe_summary(&conn, "r2", "Chicken Curry", None, None, "{}", &["indian".into()], 1000).unwrap();

        let by_name = get_recipe_summaries(&conn, Some("pasta")).unwrap();
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].id, "r1");

        let by_tag = get_recipe_summaries(&conn, Some("indian")).unwrap();
        assert_eq!(by_tag.len(), 1);
        assert_eq!(by_tag[0].id, "r2");
    }

    #[test]
    fn set_offline_available_toggles_flag_without_metadata_sync_undoing_it() {
        let conn = test_conn();
        upsert_recipe_summary(&conn, "r1", "Pasta Carbonara", None, None, "{}", &[], 1000).unwrap();

        set_offline_available(&conn, "r1", true, Some("/images/r1.jpg")).unwrap();
        let all = get_recipe_summaries(&conn, None).unwrap();
        assert!(all[0].marked_offline);
        assert_eq!(all[0].image_path.as_deref(), Some("/images/r1.jpg"));

        // A routine metadata re-sync (no image_path) must not clear the flag.
        upsert_recipe_summary(&conn, "r1", "Pasta Carbonara", None, None, "{}", &[], 2000).unwrap();
        let all = get_recipe_summaries(&conn, None).unwrap();
        assert!(all[0].marked_offline, "metadata sync must not unset marked_offline");
    }

    #[test]
    fn offline_recipe_ids_returns_only_flagged_recipes() {
        let conn = test_conn();
        upsert_recipe_summary(&conn, "r1", "A", None, None, "{}", &[], 1000).unwrap();
        upsert_recipe_summary(&conn, "r2", "B", None, None, "{}", &[], 1000).unwrap();
        set_offline_available(&conn, "r2", true, None).unwrap();

        let ids = get_offline_recipe_ids(&conn).unwrap();
        assert_eq!(ids, vec!["r2".to_string()]);
    }

    #[test]
    fn sync_meta_roundtrip() {
        let conn = test_conn();
        assert_eq!(get_sync_meta(&conn, "last_sync").unwrap(), None);
        set_sync_meta(&conn, "last_sync", "12345").unwrap();
        assert_eq!(get_sync_meta(&conn, "last_sync").unwrap(), Some("12345".to_string()));
        set_sync_meta(&conn, "last_sync", "67890").unwrap();
        assert_eq!(get_sync_meta(&conn, "last_sync").unwrap(), Some("67890".to_string()));
    }
}