# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Recipe details offline-first.** Tapping a recipe in the list opens a detail
  view built entirely from the local SQLite cache (ingredients, instructions,
  image); the full recipe is cached on first view so it is readable offline.
- **Category and tag metadata.** Categories are parsed from Mealie's
  `recipeCategory` field and surfaced in the recipe chips/labels.
- **Recipe image thumbnails.** Images are downloaded from Mealie (the
  `min-original.webp`/`original.webp` assets) and served locally via the asset
  protocol, so recipe images render offline too.
- **Shopping list tab.** Browse your Mealie shopping lists and their items from
  a new Shopping tab backed by the local cache.
  - Refresh a list (or all lists) from the server.
  - Check/uncheck items; changes are pushed back to the server with PUT, so the
    Mealie web UI stays in sync.
  - Add an individual item to a list.
  - **Add a recipe's ingredients to a list** straight from the recipe detail
    screen via Mealie's
    `POST /api/households/shopping/lists/{id}/recipe/{recipe_id}` endpoint, so
    the server handles parsing, quantities, and dedup.
- **Cargo workspace.** A root `Cargo.toml` now declares a virtual workspace with
  shared `[workspace.package]` metadata (version, edition, etc.) that
  `src-tauri/` inherits from; the lockfile moved to the workspace root.

### Changed

- Local shopping lists are pruned when a server refresh reports them deleted
  (`delete_shopping_lists_except`).

### Fixed

- `set_shopping_item_checked` now echoes the item's existing display/note fields
  in the PUT body, which prevented the server from wiping an item's display
  text and unit when toggling it.

## [0.1.0] - 2026-08-09

Initial release.

### Added

- Tauri 2 app shell (Rust core + React/TypeScript frontend) with a desktop
  window sized for a phone form factor (`420×800`).
- **Settings screen.** Server URL and long-lived Mealie token, persisted with
  `tauri-plugin-store`.
- **SQLite cache layer** (`rusqlite`, bundled) storing recipe metadata; the UI
  always reads from the cache.
- **Mealie API client** (reqwest) and a background sync engine that pulls
  recipe metadata into the cache; recipes list, search, and category labels.
- **Android support.** Build environment and `gen/android` scaffold for
  `cargo tauri android {init,dev,build}`.

[Unreleased]: https://github.com/BobBicknell/RustyMealie/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/BobBicknell/RustyMealie/releases/tag/v0.1.0