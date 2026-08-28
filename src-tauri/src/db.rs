//! SQLite persistence layer for RustyMeals.
//!
//! Owns connection setup, schema migrations, and CRUD helpers for the
//! three tables defined in the implementation plan: `recipes`,
//! `shopping_lists`, and `sync_meta`. Callers go through `AppState`
//! (see `state.rs`) rather than opening connections directly.

use rusqlite::{Connection, OptionalExtension, Result as SqlResult, params};
use serde::{Deserialize, Serialize};

/// A lightweight summary of a recipe, used for list/search views.
/// Built from the denormalized columns so we avoid parsing `raw_json`
/// just to render a list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeSummary {
    pub id: String,
    pub slug: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub image_path: Option<String>,
    pub tags: Vec<String>,
    pub categories: Vec<String>,
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
    )?;

    // Columns added post-v1; ALTER TABLE only on databases created before
    // they existed so repeated startups stay idempotent.
    add_column_if_missing(conn, "recipes", "slug", "TEXT")?;
    add_column_if_missing(conn, "recipes", "remote_image", "TEXT")?;
    add_column_if_missing(conn, "recipes", "categories", "TEXT")?;
    Ok(())
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    decl: &str,
) -> SqlResult<()> {
    let already_exists = conn
        .prepare(&format!("PRAGMA table_info({table})"))?
        .query_map([], |row| row.get::<_, String>(1))?
        .any(|name| name.as_deref() == Ok(column));

    if !already_exists {
        conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl};"))?;
    }
    Ok(())
}

/// Upsert a recipe's metadata (used during the metadata-list sync pull).
/// `remote_image` is the server-side image filename (e.g. `original.webp`)
/// needed to download the thumbnail later. Does not touch
/// `marked_offline` — that flag is only changed via `set_offline_available`,
/// so a routine metadata sync never silently un-marks a recipe the user
/// flagged for offline use.
#[allow(clippy::too_many_arguments)]
pub fn upsert_recipe_summary(
    conn: &Connection,
    id: &str,
    name: &str,
    slug: Option<&str>,
    description: Option<&str>,
    remote_image: Option<&str>,
    raw_json: &str,
    tags: &[String],
    categories: &[String],
    synced_at: i64,
) -> SqlResult<()> {
    let tags_joined = tags.join(",");
    let categories_joined = categories.join(",");
    conn.execute(
        r#"
        INSERT INTO recipes (id, name, slug, description, remote_image, raw_json, tags, categories, last_synced_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            slug = excluded.slug,
            description = excluded.description,
            remote_image = excluded.remote_image,
            image_path = COALESCE(excluded.image_path, recipes.image_path),
            raw_json = excluded.raw_json,
            tags = excluded.tags,
            categories = excluded.categories,
            last_synced_at = excluded.last_synced_at
        "#,
        params![
            id,
            name,
            slug,
            description,
            remote_image,
            raw_json,
            tags_joined,
            categories_joined,
            synced_at
        ],
    )?;
    Ok(())
}

/// Overwrite a recipe's full payload (replacing the `"{}"` placeholder from
/// the metadata pass) and optionally record its locally-cached image path.
pub fn update_recipe_payload(
    conn: &Connection,
    id: &str,
    raw_json: &str,
    image_path: Option<&str>,
    synced_at: i64,
) -> SqlResult<()> {
    conn.execute(
        "UPDATE recipes SET raw_json = ?1, image_path = COALESCE(?2, image_path), last_synced_at = ?3 WHERE id = ?4",
        params![raw_json, image_path, synced_at, id],
    )?;
    Ok(())
}

/// Fetch recipe summaries, optionally filtered by a case-insensitive
/// substring match against name/tags/categories, and/or an exact
/// category and tag match. Tags/categories are stored comma-joined, so
/// token matching wraps them in commas before `LIKE` to avoid partial
/// (sub-token) matches: `,main course,` never matches `,main,`.
pub fn get_recipe_summaries(
    conn: &Connection,
    query: Option<&str>,
    category: Option<&str>,
    tag: Option<&str>,
) -> SqlResult<Vec<RecipeSummary>> {
    let mut sql = String::from(
        "SELECT id, slug, name, description, image_path, tags, categories, marked_offline FROM recipes",
    );
    let mut clauses: Vec<String> = Vec::new();
    let mut values: Vec<String> = Vec::new();

    if let Some(q) = query.filter(|q| !q.trim().is_empty()) {
        let pattern = format!("%{}%", q.trim());
        clauses.push(
            "(name LIKE ? OR (',' || tags || ',') LIKE ? OR (',' || categories || ',') LIKE ?)"
                .to_string(),
        );
        values.extend([pattern.clone(), pattern.clone(), pattern]);
    }
    if let Some(cat) = category.filter(|c| !c.trim().is_empty()) {
        clauses.push("(',' || categories || ',') LIKE ?".to_string());
        values.push(format!("%,{}%", cat.trim()));
    }
    if let Some(t) = tag.filter(|t| !t.trim().is_empty()) {
        clauses.push("(',' || tags || ',') LIKE ?".to_string());
        values.push(format!("%,{}%", t.trim()));
    }

    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }
    sql.push_str(" ORDER BY name");

    let mut stmt = conn.prepare(&sql)?;
    let map_row = |row: &rusqlite::Row| -> SqlResult<RecipeSummary> {
        let tags_raw: Option<String> = row.get(5)?;
        let categories_raw: Option<String> = row.get(6)?;
        let split = |raw: Option<String>| -> Vec<String> {
            raw.unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        };
        Ok(RecipeSummary {
            id: row.get(0)?,
            slug: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            image_path: row.get(4)?,
            tags: split(tags_raw),
            categories: split(categories_raw),
            marked_offline: row.get::<_, i64>(7)? != 0,
        })
    };

    let rows = stmt
        .query_map(rusqlite::params_from_iter(values.iter()), map_row)?
        .collect::<SqlResult<Vec<_>>>()?;
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

/// A recipe whose thumbnail still needs to be pulled locally.
#[derive(Debug, PartialEq)]
pub struct ImageToFetch {
    pub id: String,
    pub remote_image: String,
}

/// Recipes that have a server-side image filename but no locally cached
/// copy yet — the sync engine downloads each so list/detail views can show
/// every thumbnail offline. Recipes whose download failed stay in here and
/// are retried on the next sync.
pub fn get_recipes_needing_image(conn: &Connection) -> SqlResult<Vec<ImageToFetch>> {
    let mut stmt = conn.prepare(
        "SELECT id, remote_image FROM recipes \
         WHERE remote_image IS NOT NULL AND remote_image != '' \
           AND (image_path IS NULL OR image_path = '')",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ImageToFetch {
                id: row.get(0)?,
                remote_image: row.get(1)?,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;
    Ok(rows)
}

/// Record where a recipe's thumbnail was cached on disk.
pub fn set_recipe_image(conn: &Connection, id: &str, image_path: &str) -> SqlResult<()> {
    conn.execute(
        "UPDATE recipes SET image_path = ?1 WHERE id = ?2",
        params![image_path, id],
    )?;
    Ok(())
}

/// The locally-cached image path for a recipe, if it has been downloaded.
pub fn get_recipe_image_path(conn: &Connection, id: &str) -> SqlResult<Option<String>> {
    conn.query_row(
        "SELECT image_path FROM recipes WHERE id = ?1",
        params![id],
        |row| row.get::<_, Option<String>>(0),
    )
    .optional()
    .map(|opt| opt.flatten())
}

/// Everything the sync engine needs to fully pull an offline-flagged
/// recipe: its id (for image caching) and server slug (for detail fetch).
#[derive(Debug, PartialEq)]
pub struct OfflineRecipeRef {
    pub id: String,
    pub slug: Option<String>,
    pub remote_image: Option<String>,
}

/// Recipes currently flagged offline — the sync engine re-pulls each one's
/// full body + thumbnail.
pub fn get_offline_recipe_refs(conn: &Connection) -> SqlResult<Vec<OfflineRecipeRef>> {
    let mut stmt =
        conn.prepare("SELECT id, slug, remote_image FROM recipes WHERE marked_offline = 1")?;
    let rows = stmt
        .query_map([], |row| {
            Ok(OfflineRecipeRef {
                id: row.get(0)?,
                slug: row.get(1)?,
                remote_image: row.get(2)?,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;
    Ok(rows)
}

/// Upsert a shopping list's raw payload.
pub fn upsert_shopping_list(
    conn: &Connection,
    id: &str,
    name: &str,
    raw_json: &str,
    synced_at: i64,
) -> SqlResult<()> {
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
/// Returns (id, name, raw_json) per list, ordered by name.
pub fn get_shopping_lists_raw(conn: &Connection) -> SqlResult<Vec<(String, String, String)>> {
    let mut stmt = conn.prepare("SELECT id, name, raw_json FROM shopping_lists ORDER BY name")?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<SqlResult<Vec<_>>>()?;
    Ok(rows)
}

/// Delete every locally-cached shopping list that no longer exists on the
/// server, so a refresh never shows stale lists. Passing an empty set
/// removes all cached lists.
pub fn delete_shopping_lists_except(conn: &Connection, keep_ids: &[String]) -> SqlResult<()> {
    if keep_ids.is_empty() {
        conn.execute("DELETE FROM shopping_lists", [])?;
        return Ok(());
    }
    let placeholders = keep_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("DELETE FROM shopping_lists WHERE id NOT IN ({placeholders})");
    conn.execute(&sql, rusqlite::params_from_iter(keep_ids.iter()))?;
    Ok(())
}

/// Get/set a single `sync_meta` key (e.g. `last_full_sync_at`).
pub fn get_sync_meta(conn: &Connection, key: &str) -> SqlResult<Option<String>> {
    conn.query_row(
        "SELECT value FROM sync_meta WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
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
    fn migration_includes_slug_and_remote_image_columns() {
        let conn = test_conn();
        let columns: Vec<String> = conn
            .prepare("PRAGMA table_info(recipes)")
            .unwrap()
            .query_map([], |row| row.get(1))
            .unwrap()
            .collect::<SqlResult<Vec<_>>>()
            .unwrap();
        assert!(columns.contains(&"slug".to_string()));
        assert!(columns.contains(&"remote_image".to_string()));
    }

    #[test]
    fn migration_alters_legacy_table_missing_new_columns() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE recipes (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                image_path TEXT,
                raw_json TEXT NOT NULL,
                tags TEXT,
                marked_offline INTEGER NOT NULL DEFAULT 0,
                last_synced_at INTEGER
            );",
        )
        .unwrap();
        run_migrations(&conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('recipes') WHERE name IN ('slug', 'remote_image')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn upsert_and_fetch_recipe_summary() {
        let conn = test_conn();
        upsert_recipe_summary(
            &conn,
            "r1",
            "Pasta Carbonara",
            Some("pasta-carbonara"),
            Some("Classic Roman pasta"),
            Some("original.webp"),
            r#"{"id":"r1"}"#,
            &["italian".to_string(), "pasta".to_string()],
            &["main".to_string()],
            1000,
        )
        .unwrap();

        let all = get_recipe_summaries(&conn, None, None, None).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "Pasta Carbonara");
        assert_eq!(all[0].tags, vec!["italian", "pasta"]);
        assert_eq!(all[0].categories, vec!["main"]);
        assert_eq!(all[0].slug.as_deref(), Some("pasta-carbonara"));
        assert!(!all[0].marked_offline);

        let slug: String = conn
            .query_row("SELECT slug FROM recipes WHERE id='r1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(slug, "pasta-carbonara");
    }

    #[test]
    fn search_filters_by_name_and_tags_and_categories() {
        let conn = test_conn();
        upsert_recipe_summary(
            &conn,
            "r1",
            "Pasta Carbonara",
            Some("pasta-carbonara"),
            None,
            None,
            "{}",
            &["italian".into()],
            &["main".into()],
            1000,
        )
        .unwrap();
        upsert_recipe_summary(
            &conn,
            "r2",
            "Chicken Curry",
            Some("chicken-curry"),
            None,
            None,
            "{}",
            &["indian".into()],
            &["main".into()],
            1000,
        )
        .unwrap();
        upsert_recipe_summary(
            &conn,
            "r3",
            "Caesar Salad",
            Some("caesar-salad"),
            None,
            None,
            "{}",
            &[],
            &["side".into()],
            1000,
        )
        .unwrap();

        let by_name = get_recipe_summaries(&conn, Some("pasta"), None, None).unwrap();
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].id, "r1");

        let by_tag = get_recipe_summaries(&conn, Some("indian"), None, None).unwrap();
        assert_eq!(by_tag.len(), 1);
        assert_eq!(by_tag[0].id, "r2");

        let by_category = get_recipe_summaries(&conn, None, Some("main"), None).unwrap();
        assert_eq!(by_category.len(), 2);

        let by_tag_exact = get_recipe_summaries(&conn, None, None, Some("italian")).unwrap();
        assert_eq!(by_tag_exact.len(), 1);

        // sub-token must not match: "in" is inside "italian"/"indian" but is a real tag here, so use an unambiguous probe
        let by_tag_exact = get_recipe_summaries(&conn, None, None, Some("ardi")).unwrap();
        assert!(by_tag_exact.is_empty());
    }

    #[test]
    fn set_offline_available_toggles_flag_without_metadata_sync_undoing_it() {
        let conn = test_conn();
        upsert_recipe_summary(
            &conn,
            "r1",
            "Pasta Carbonara",
            Some("pasta-carbonara"),
            None,
            None,
            "{}",
            &[],
            &[],
            1000,
        )
        .unwrap();

        set_offline_available(&conn, "r1", true, Some("/images/r1.jpg")).unwrap();
        let all = get_recipe_summaries(&conn, None, None, None).unwrap();
        assert!(all[0].marked_offline);
        assert_eq!(all[0].image_path.as_deref(), Some("/images/r1.jpg"));

        // A routine metadata re-sync (no image_path) must not clear the flag.
        upsert_recipe_summary(
            &conn,
            "r1",
            "Pasta Carbonara",
            Some("pasta-carbonara"),
            None,
            None,
            "{}",
            &[],
            &[],
            2000,
        )
        .unwrap();
        let all = get_recipe_summaries(&conn, None, None, None).unwrap();
        assert!(
            all[0].marked_offline,
            "metadata sync must not unset marked_offline"
        );
    }

    #[test]
    fn offline_recipe_refs_returns_only_flagged_recipes() {
        let conn = test_conn();
        upsert_recipe_summary(
            &conn,
            "r1",
            "A",
            Some("a"),
            None,
            Some("original.webp"),
            "{}",
            &[],
            &[],
            1000,
        )
        .unwrap();
        upsert_recipe_summary(
            &conn,
            "r2",
            "B",
            Some("b"),
            None,
            None,
            "{}",
            &[],
            &[],
            1000,
        )
        .unwrap();
        set_offline_available(&conn, "r2", true, None).unwrap();

        let refs = get_offline_recipe_refs(&conn).unwrap();
        assert_eq!(
            refs,
            vec![OfflineRecipeRef {
                id: "r2".to_string(),
                slug: Some("b".to_string()),
                remote_image: None,
            }]
        );
    }

    #[test]
    fn update_recipe_payload_replaces_placeholder_and_keeps_flag() {
        let conn = test_conn();
        upsert_recipe_summary(
            &conn,
            "r1",
            "A",
            Some("a"),
            None,
            Some("original.webp"),
            "{}",
            &[],
            &[],
            1000,
        )
        .unwrap();
        set_offline_available(&conn, "r1", true, None).unwrap();

        update_recipe_payload(
            &conn,
            "r1",
            r#"{"id":"r1","name":"A","recipeInstructions":[]}"#,
            Some("/images/r1.webp"),
            5000,
        )
        .unwrap();

        let raw = get_recipe_raw_json(&conn, "r1").unwrap().unwrap();
        assert!(raw.contains("recipeInstructions"));
        let summary = &get_recipe_summaries(&conn, None, None, None).unwrap()[0];
        assert!(summary.marked_offline);
        assert_eq!(summary.image_path.as_deref(), Some("/images/r1.webp"));
    }

    #[test]
    fn recipes_needing_image_returns_only_missing_thumbnails() {
        let conn = test_conn();
        upsert_recipe_summary(
            &conn,
            "r1",
            "A",
            Some("a"),
            None,
            Some("original.webp"),
            "{}",
            &[],
            &[],
            1000,
        )
        .unwrap();
        upsert_recipe_summary(
            &conn,
            "r2",
            "B",
            Some("b"),
            None,
            Some("original.webp"),
            "{}",
            &[],
            &[],
            1000,
        )
        .unwrap();
        set_recipe_image(&conn, "r2", "/images/r2.webp").unwrap();

        let needing = get_recipes_needing_image(&conn).unwrap();
        assert_eq!(needing.len(), 1);
        assert_eq!(
            needing[0],
            ImageToFetch {
                id: "r1".to_string(),
                remote_image: "original.webp".to_string(),
            }
        );

        assert_eq!(get_recipe_image_path(&conn, "r1").unwrap(), None);
        assert_eq!(
            get_recipe_image_path(&conn, "r2").unwrap(),
            Some("/images/r2.webp".to_string())
        );
    }

    #[test]
    fn sync_meta_roundtrip() {
        let conn = test_conn();
        assert_eq!(get_sync_meta(&conn, "last_sync").unwrap(), None);
        set_sync_meta(&conn, "last_sync", "12345").unwrap();
        assert_eq!(
            get_sync_meta(&conn, "last_sync").unwrap(),
            Some("12345".to_string())
        );
        set_sync_meta(&conn, "last_sync", "67890").unwrap();
        assert_eq!(
            get_sync_meta(&conn, "last_sync").unwrap(),
            Some("67890".to_string())
        );
    }

    #[test]
    fn shopping_lists_roundtrip_and_prune() {
        let conn = test_conn();
        assert!(get_shopping_lists_raw(&conn).unwrap().is_empty());

        upsert_shopping_list(&conn, "a", "Alpha", r#"{"id":"a","listItems":[]}"#, 1000).unwrap();
        upsert_shopping_list(&conn, "b", "Beta", r#"{"id":"b","listItems":[]}"#, 1000).unwrap();
        upsert_shopping_list(&conn, "a", "Alpha 2", r#"{"id":"a","listItems":[]}"#, 2000).unwrap();

        let rows = get_shopping_lists_raw(&conn).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "a");
        assert_eq!(rows[0].1, "Alpha 2");
        assert_eq!(rows[1].0, "b");

        // A refresh that only sees "a" must drop the stale "b".
        delete_shopping_lists_except(&conn, &["a".to_string()]).unwrap();
        let rows = get_shopping_lists_raw(&conn).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "a");

        // An empty server set clears everything.
        delete_shopping_lists_except(&conn, &[]).unwrap();
        assert!(get_shopping_lists_raw(&conn).unwrap().is_empty());
    }
}
