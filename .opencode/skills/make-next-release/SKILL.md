---
name: make-next-release
description: Make the next RustyMealie release — figure out the current dev version in Cargo.toml, update the CHANGELOG, pin that version for release, merge development into main, push, tag v<version>, then roll development back to the next -test.N dev version. Use when the user says "make the next release", "next release", "cut a release", "do/release vX.Y.Z", "ship/tag/publish", or wants to bump the version and run the Release workflow.
---

# Make Next Release

End-to-end next release: update the CHANGELOG, promote the current
`development` state to a clean released version, merge → push → tag (which
triggers the GitHub Actions Release workflow), then return `development` to a
`-test.N` dev version ready for the following iteration.

## How the automation works (read this first)

- `.github/workflows/release.yml` on GitHub runs **on push to `main`** (not on
  tags) and:
  1. Reads the version from the first `version = "..."` in the top-level
     `Cargo.toml` (`[workspace.package]`).
  2. **Gates on it: if the version string contains `test`, the release build is
     skipped entirely** (`allow=false`). Otherwise it builds desktop bundles.
  3. Publishes a GitHub Release with tag `v<version>`
     (`softprops/action-gh-release`, `skip_if_release_exists: true`).
- The app's on-screen version (Settings → `v...`) comes from
  `env!("CARGO_PKG_VERSION")` in `src-tauri/src/lib.rs`, which — via
  `version.workspace = true` in `src-tauri/Cargo.toml` — is always the top-level
  `Cargo.toml` version. Bump the workspace version and the display follows.
- `src-tauri/tauri.conf.json` `version` is a separate channel: it drives the
  Android `versionName`/`versionCode`. Keep it equal to the **released**
  version `<version>`; the dev `-test.N` suffix lives only in `Cargo.toml`.

## Version conventions

- `development` always carries a dev version with a `-test.N` suffix, e.g.
  `0.2.1-test.1`. It never matches a released version `X.Y.Z` (that would
  accidentally pass the gate).
- **Derive the release version from the current dev version**: strip the
  `-test.N` suffix. `0.2.1-test.1` → release **`0.2.1`**. Check the top-level
  `Cargo.toml` `[workspace.package]` version and read the number off it rather
  than guessing.
- A release pins the version to clean semver: `0.2.0`, `0.2.1`, `0.3.0`, …
- After releasing `X.Y.Z`, the next dev version is **`X.Y.(Z+1)-test.1`**
  (patch bump + suffix), e.g. release `0.2.1` → dev `0.2.2-test.1`. Never
  `0.2.1-test.1` reused for the same number being released — that collides.

## Procedure

Confirm branch and clean state first:

```bash
git branch --show-current        # must be: development
git status --short
```

Any local changes must be intentional and committed; round up any leftover
`git checkout -- <file>` for tooling noise (e.g. `.idea/` churn from IDE
scaffolding) before opening the release commit.

### 1. Update the CHANGELOG (`CHANGELOG.md`)

- Rename `## [Unreleased]` → `## [<version>] - <YYYY-MM-DD>` (today's date).
- Fold this release's user-visible changes into the `Added` / `Changed` /
  `Fixed` sections under it; remove or re-home anything that did not actually
  ship. Keep the Keep-a-Changelog + Semantic Versioning intro intact.
- Add a fresh `## [Unreleased]` heading directly above the new release section
  for upcoming work.
- Update the reference links at the bottom of the file:
  - `[Unreleased]` → `.../compare/v<version>...HEAD`
  - add `[<version>]: https://github.com/BobBicknell/RustyMealie/releases/tag/v<version>`

### 2. Pin the version for release

Top-level `Cargo.toml`, `[workspace.package]`:

```toml
[workspace.package]
version = "0.2.0"     # NO -test suffix, or CI skips the build
```

Run `cargo check` (in `src-tauri/`) so `Cargo.lock` updates the crate version.
Also confirm `src-tauri/tauri.conf.json` `version` equals the same clean version.

### 3. Commit on `development`

`git add` only the intended files (never `.idea/`), including
`Cargo.toml`, `Cargo.lock`, `src-tauri/tauri.conf.json`, `CHANGELOG.md`, and any
code changes not yet committed.

```bash
git commit -m "chore(release): v<version>"
```

Follow the existing conventional-commit style if more than a version bump is
involved (e.g. `fix(...)`, `feat(...)` lines in the body).

### 4. Merge to `main`

```bash
git checkout main
git fetch origin
git log --oneline development..main   # expect EMPTY output
```

If `main` has nothing unique, fast-forward merge is clean and expected:

```bash
git merge --ff-only development
```

If `development..main` is non-empty, `--ff-only` will fail — stop and reconcile
(a merge commit + conflict resolution) before pushing, rather than forcing.

### 5. Push `main` and tag

Pushing `main` is what triggers the Release workflow.

```bash
git push origin main
git tag v<version>                    # e.g. v0.2.0, matches workflow's tag
git push origin v<version>
```

Tag on the merge commit and name it exactly `v<version>` (the workflow looks for
that name). `skip_if_release_exists: true` dedupes if the workflow races the
tag.

### 6. Verify the release kicked off

gh (if authenticated) or the unauthenticated API — the release appears once the
build finishes (a few minutes):

```bash
gh run list --workflow=release.yml --limit 2
curl -s https://api.github.com/repos/BobBicknell/RustyMealie/releases/tags/v<version>
```

If not up after a while, check the Actions tab for the gate output ("version"
vs "allow").

### 7. Reset `development` to a dev version

```bash
git checkout development
```

Patch-bump the workspace version + suffix (`0.2.0` → `0.2.1-test.1`), run
`cargo check` to refresh `Cargo.lock`, and commit/push:

```bash
git commit -m "chore(version): bump workspace version to 0.2.1-test.1 for development"
git push origin development
```

## Caveats

- **The tag does not run the workflow; pushing `main` does.** The tag only
  marks the commit and pre-names the GitHub release.
- The release workflow builds **desktop** bundles only (`.deb`, `.AppImage`).
  It does **not** build the Android APK — that is a local step
  (`cargo tauri android build --apk --debug` + `adb install -r`) if you want to
  ship the phone build too.
- The gate string-searchs for `test`, so any dev version without a suffix
  pushed to `main` will publish — when in doubt, leave the suffix on
  `development`.
- Don't compile-release-changes before tagging: tag exactly the commit you
  pushed. If a release build fails that gate or the merge is dirty, fix on
  `development`, re-merge, re-push, `git tag -f`/delete-remote-tag, and re-push
  the tag on the corrected commit.
- `.idea/` (JetBrains) files are tracked but should never enter a release
  commit — unstage/checkout their churn.
- Android emulator device `adb` links are flaky post-resume: re-`adb
  wait-for-device` before install; never imply the phone is gone.