# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.7] - 2026-08-30

### Added

- **Pre-push checks for the frontend.** The git pre-push hook now also runs
  `npm audit`, the TypeScript typecheck (`tsc --noEmit`), and the new `oxlint`
  linter on the React codebase, in addition to `cargo audit` on the Rust side.

### Changed

- **More helpful error messages.** When the Mealie server returns an error,
  the response body is now included alongside the HTTP status code, making sync
  and API failures far easier to diagnose.

## [0.2.6] - 2026-08-29

### Fixed

- **Faster, smoother sync.** Recipe metadata writes are batched into a single
  database transaction instead of one transaction per recipe, and sync progress
  now updates every 10 recipes rather than only at the start and end.
- **Vite dev-server vulnerability.** Bumped Vite to 6.4.3 to resolve
  Dependabot-flagged dev-server path-traversal advisories (GHSA-4w7w-66w2-5vf9
  and a related esbuild advisory); `npm audit` now reports 0 vulnerabilities.

## [0.2.5] - 2026-08-29

### Security

- **h2 vulnerability fixed.** Bumped the `h2` dependency to 0.4.19 to resolve
  RUSTSEC-2026-0258 (unbounded empty DATA frames / denial of service).
- **Pre-push audit hook.** Added a git pre-push hook that runs `cargo audit`
  and blocks pushes that contain known dependency vulnerabilities.

## [0.2.4] - 2026-08-29

### Added

- **Remove checked items.** Shopping lists now show a "Remove N ✓" button in
  the list header when one or more items are crossed off, permanently deleting
  them on the server via Mealie's bulk delete endpoint and refreshing the local
  cache.

### Fixed

- **Add input overflow.** The "Add" field and button on the shopping screen no
  longer run off the right edge of the card on narrow screens.
- **Android status bar overlap.** Page content is now padded below the status
  bar/notch area (`safe-area-inset-top`) instead of sitting underneath it.
- **Dev watcher crash.** The Vite dev server no longer watches the Cargo build
  output directories, which previously exhausted the inotify file-watcher limit
  and crashed `npm run tauri dev` with `ENOSPC`.

### Changed

- **CI on GitHub Actions v5.** Workflows upgraded past the deprecated Node 20
  runtime.

## [0.2.3] - 2026-08-29

### Changed

- **Smaller Android APK.** Releases now ship a single signed **release** APK
  (`app-universal-release.apk`, ~65 MB) instead of the enormous debug build
  (`app-universal-debug.apk`, ~668 MB). The release build optimizes and strips
  the Rust native libraries across all four ABIs while remaining sideloadable
  via `adb install -r`.

### Fixed

- **Release tagging.** The release workflow now builds the GitHub release tag
  and name from the gated version correctly, instead of publishing a broken
  release tagged just `v`.

## [0.2.2] - 2026-08-29

### Added

- **Android APK in releases.** Releases now include a debug-signed Android
  APK (`app-universal-debug.apk`) alongside the desktop bundles, built
  automatically by the release workflow, so the app can be sideloaded onto an
  Android phone via `adb install -r`.

### Removed

- The `.deb` bundle is no longer published; the portable AppImage remains for
  Linux desktop users.

## [0.2.1] - 2026-08-29

Maintenance release with no user-visible changes; corrects `cargo fmt`
formatting in the sync module so the CI formatting check passes.

## [0.2.0] - 2026-08-29

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
- **Live sync progress.** Recipe sync now emits progress events and the Settings
  screen shows a progress bar with a live recipe/thumbnail counter and
  "Connecting to server…" state instead of hanging silently.
- **App version display.** The Settings screen shows the app version, sourced
  from the workspace `Cargo.toml` at compile time.

### Changed

- **HTTPS on Android.** reqwest is downgraded to 0.12 with bundled Mozilla
  roots (`rustls-tls-webpki-roots`) so sync works over TLS on Android without
  `rustls-platform-verifier`, which panicked on the first HTTPS request.
- **Image downloads.** Images are fetched in parallel batches with a per-image
  timeout, plus 10 s connect / 30 s request timeouts on the Mealie client.
- Local shopping lists are pruned when a server refresh reports them deleted
  (`delete_shopping_lists_except`).

### Fixed

- **Bottom navigation overlap.** The tab bar no longer overlaps the Android
  system gesture/navigation bar (`viewport-fit=cover` + safe-area insets).
- **"Add to list" list picker.** The shopping-list picker on the recipe detail
  screen rendered off-screen and appeared to do nothing; it is now a bottom
  sheet modal that reliably opens.
- **Shopping tab refresh loop.** The page could get stuck showing
  "Refreshing…" forever; it now refreshes exactly once when opened.
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

[Unreleased]: https://github.com/BobBicknell/RustyMealie/compare/v0.2.7...HEAD
[0.2.7]: https://github.com/BobBicknell/RustyMealie/releases/tag/v0.2.7
[0.2.6]: https://github.com/BobBicknell/RustyMealie/releases/tag/v0.2.6
[0.2.5]: https://github.com/BobBicknell/RustyMealie/releases/tag/v0.2.5
[0.2.4]: https://github.com/BobBicknell/RustyMealie/releases/tag/v0.2.4
[0.2.3]: https://github.com/BobBicknell/RustyMealie/releases/tag/v0.2.3
[0.2.2]: https://github.com/BobBicknell/RustyMealie/releases/tag/v0.2.2
[0.2.1]: https://github.com/BobBicknell/RustyMealie/releases/tag/v0.2.1
[0.2.0]: https://github.com/BobBicknell/RustyMealie/releases/tag/v0.2.0
[0.1.0]: https://github.com/BobBicknell/RustyMealie/releases/tag/v0.1.0