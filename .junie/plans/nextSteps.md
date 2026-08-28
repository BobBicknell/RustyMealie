# RustyMealie - Next Steps

## [x] Step 5: Frontend Sync Integration
- [x] Add `triggerSync` to `src/services/db.js` to wrap the new Tauri command bridge.
- [x] Create a `SyncSettingsScreen.jsx` component containing input forms for `base_url` and `token`.
- [x] Add a loading spinner state to indicate when background data fetching is active.

## [x] Step 6: Expand Offline Sync Execution
- [x] Update `trigger_sync` in `lib.rs` to fetch full recipe details instead of placeholders (`"{}"`).
- [x] Add image download processing to cache food thumbnails to local device storage.
- [x] Wire up sync tracking parameters into the SQLite `sync_meta` data layout.

## [ ] Step 7: Recipe Detail Screen & Offline Toggle UI
- [ ] Create a `RecipeDetailScreen.jsx` that renders ingredients/steps from `raw_json` via `get_recipe_detail`.
- [ ] Wire the "Mark offline" toggle to `toggle_offline_recipe` and re-sync flagged recipes.
