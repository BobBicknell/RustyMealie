# RustyMeals

Offline-first client for a self-hosted [Mealie](https://mealie.io/) instance,
built with **Tauri 2** (Rust core) and a **React + TypeScript** frontend.

The desktop window is sized for a phone form factor, and the same core targets
Android. Source is at [github.com/BobBicknell/RustyMealie](https://github.com/BobBicknell/RustyMealie).

## Features

- **Recipes**
  - Browse, search, and filter your Mealie recipes (by category/tagged with
    `recipeCategory`, with image thumbnails).
  - Open a full recipe detail view — ingredients, instructions, image — built
    entirely from the local cache, so it renders offline.
- **Shopping lists**
  - Browse all of your Mealie shopping lists and their items from a dedicated
    tab.
  - Check items off; the state is pushed back to the server, so the Mealie web
    UI stays in sync.
  - Add individual items, or add a whole recipe's ingredients to a list right
    from the recipe detail screen.
- **Offline-first.** The React UI never talks to Mealie directly — all network
  traffic and persistence happens in the Rust core. The UI always reads from a
  local SQLite cache, which is what makes offline behaviour deterministic.

## Architecture

```
React/TypeScript UI ──▶ Tauri commands (lib.rs) ──▶ db.rs ─(to)─▶ SQLite cache
                                                      │ API  │
                                                      └── api.rs ─(to)─▶ Mealie REST API
```

The UI calls Rust commands via `invoke()`. Commands read/write the SQLite cache
(`db.rs`) and, when online, talk to Mealie through the HTTP client (`api.rs`).
Cached data is always served to the UI first; network calls happen in the
background on the app's sync refresh.

## Project layout

```
/ (repo root)                # Cargo workspace + Vite/React/TS frontend
  Cargo.toml                 # virtual workspace; shared package metadata
  Cargo.lock
  package.json, vite.config.ts, tailwind.config.js, index.html
  src/                       # React app
    screens/                 #   RecipeList, RecipeDetail, ShoppingList, Settings
    components/              #   shared components (RecipeCard, etc.)
    services/db.ts           #   typed wrapper over the Tauri commands
    App.tsx                  #   tab navigation
  src-tauri/                 # Rust core
    src/{main.rs,lib.rs,api.rs,db.rs,state.rs}
    tauri.conf.json, build.rs, capabilities/
    gen/android/             #   Android project scaffold
```

## Prerequisites

- Rust (stable) — see the minimum supported version in the root `Cargo.toml`
- Node.js 18+ / npm
- Tauri CLI v2:
  ```
  cargo install tauri-cli --version "^2.0.0"
  ```
- Android targets (only needed for mobile builds):
  ```
  rustup target add aarch64-linux-android armv7-linux-androideabi \
      i686-linux-android x86_64-linux-android
  ```
- Android Studio SDK + NDK for `android` commands, with `ANDROID_HOME`.

## Development

Install frontend dependencies:

```
npm install
```

Run the desktop dev build (hot-reloads UI + Rust core):

```
npm run tauri dev
```

Verify the frontend (types + production build):

```
npm run typecheck && npm run build
```

Verify the Rust core — run from the repo root (workspace):

```
cargo check
cargo test
```

## Android

One-time scaffold of the Android project (creates `src-tauri/gen/android`):

```
cargo tauri android init
```

Run on an emulator or connected device:

```
cargo tauri android dev
```

Build a release APK/AAB:

```
cargo tauri android build
```

## Configuration

The Mealie server URL and auth token are entered on the Settings screen and
stored via `tauri-plugin-store` in a `settings.json` file inside the app data
directory:

- macOS/Linux: `~/.local/share/com.bicknell.rustymeals/settings.json`

> **Note:** the token is stored as plaintext JSON on disk (standard for
> `tauri-plugin-store`). It is **not** kept in the SQLite cache, but the store
> file itself is not encrypted.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for release history. Bump the version in the
root `Cargo.toml` (`[workspace.package]`).