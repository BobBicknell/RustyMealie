# RustyMealie - Next Steps

## [ ] Step 5: Frontend Sync Integration
- [ ] Add `triggerSync` to `src/services/db.js` to wrap the new Tauri command bridge.
- [ ] Create a `SyncSettingsScreen.jsx` component containing input forms for `base_url` and `token`.
- [ ] Add a loading spinner state to indicate when background data fetching is active.

## [ ] Step 6: Expand Offline Sync Execution
- [ ] Update `trigger_sync` in `lib.rs` to fetch full recipe details instead of placeholders (`"{}"`).
- [ ] Add image download processing to cache food thumbnails to local device storage.
- [ ] Wire up sync tracking parameters into the SQLite `sync_meta` data layout.
