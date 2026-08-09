# RustyMeals

Offline-first Android (and eventually iOS) client for a self-hosted
[Mealie](https://mealie.io/) instance, built with **Tauri 2.0** (Rust core +
React/TypeScript frontend).

The React UI never talks to Mealie directly — all network + persistence goes
through Rust Tauri commands, and the UI always reads from a local SQLite cache.
That is what makes offline behaviour deterministic.

## Project layout

```
/ (repo root)              # Vite + React + TypeScript frontend
  package.json, vite.config.ts, tailwind.config.js, index.html
  src/                     # React app (screens, components, api wrapper)
  src-tauri/               # Rust core
    Cargo.toml, tauri.conf.json, build.rs
    src/{main.rs,lib.rs,state.rs,db.rs,api.rs,sync.rs,commands.rs}
    gen/android/           # created by `cargo tauri android init`
```

## Prerequisites

- Rust (stable) with the mobile targets:
  ```
  rustup target add aarch64-linux-android armv7-linux-androideabi \
      i686-linux-android x86_64-linux-android
  ```
- Node.js 18+ / npm
- Tauri CLI v2:
  ```
  cargo install tauri-cli --version "^2.0.0"
  ```
- For Android builds: Android Studio with the SDK + NDK installed, and the
  `ANDROID_HOME` and `NDK_HOME` environment variables set.

## Development

Install frontend dependencies:

```
npm install
```

Run the desktop dev build (fast iteration on the UI + Rust core):

```
npm run tauri dev
```

Type-check / build just the frontend:

```
npm run build
```

Check the Rust core:

```
cd src-tauri && cargo check
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

The default Mealie server URL is entered on the Login screen and stored
locally; the auth token is kept in the secure Tauri store (never in plain
SQLite).
