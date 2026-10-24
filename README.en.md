# RepoLens

[简体中文](README.md) | **English**

A local workbench for checking Git commit identities across multiple repositories. See configured names, emails, and their sources, check each entry against identity rules, and safely edit repository configuration after reviewing a preview.

Current version: `0.1.0-alpha.1`, an internal validation build. The application currently uses Simplified Chinese and a light theme. This English README does not imply an English application interface.

![RepoLens current interface preview](docs/screenshots/workspace-web-preview.png)

This is the current browser layout preview, without mock repository data. Reading repositories, scanning, and saving require the desktop application. Earlier internal screenshots of the [desktop workbench](docs/screenshots/workspace.png) and [change preview](docs/screenshots/preview.png) remain available as workflow references; their layouts may differ from the current version.

## Features

- **Discover and organize repositories:** Add a repository directly or scan directories and select results to collect. Supports regular repositories, bare repositories, linked worktrees, and initialized submodules. Configure scan depth and exclusions; cancellation or scan failure preserves discovered results.
- **Review repository status:** Inspect HEAD, branches, working tree status, recent commits, upstream, and remotes. Organize entries into groups, search, and filter. A compact summary shows collected, passed, and attention counts, with details in a side panel.
- **Check commit identities:** Read names, emails, and configuration sources for `user`, `author`, and `committer` separately. Set an expected name, email, and allowed domains per entry. Failed reads, confirmed absence, and explicitly empty values remain distinct; unknown results never count as a pass.
- **Preview configuration edits:** Edit or remove only repository-local `user.name` and `user.email`. Review differences, overrides, and known shared effects before confirming. Configuration changes, including conditional include declarations, invalidate an old preview. Saved values are read back for verification.
- **Inspect authentication configuration:** View remote addresses, HTTPS credential helpers, and static SSH metadata. Address secrets are redacted in the backend; configuration presence does not prove successful authentication.
- **Use the updated interface:** Shared design tokens, a compact layout, first-use guidance, status and configuration-scope tooltips, and repository selection with the up/down arrow keys and `Esc` to close details.

## Use the desktop application

Only local internal builds are available; there is no formal installer release yet. After building on Windows, run `src-tauri/target/release/repolens.exe` directly or use the installer under `src-tauri/target/release/bundle/nsis/`. Runtime requirements are a local Git 2.46+ installation and WebView2 Runtime. Current installers are unsigned.

1. Add a repository, or scan a project directory and select results to collect. The default scan depth is 3; scanning does not automatically collect repositories.
2. Select an entry to inspect its overview, commit identity, and authentication configuration. Set identity expectations in the entry and rules dialog.
3. Edit the base identity or remove local configuration, review the preview, confirm the save, and inspect the values read back afterward.

Rules belong to individual entries; groups do not inherit rules. Having no rules does not by itself require attention, but missing identities still do. Linked worktrees may share local configuration; the preview lists known related entries already collected in the application.

The application operates offline by default. It does not perform daily Git operations such as commit, fetch, pull, or push. Authentication inspection does not retrieve passwords or tokens, execute credential helpers or SSH proxy commands, or open private keys. Global identity editing, author/committer or worktree-specific identity editing, authentication connection tests, a dark theme, and automatic updates are currently unavailable.

Removing an entry only removes its application record, leaving the repository on disk. SQLite stores entries, groups, rules, and the Git executable path. Find the application data directory in settings. See the [implementation record](docs/IMPLEMENTATION.md) for backup and uninstall details.

## Local development

Requires Node.js 22.12+, a Rust toolchain, local Git 2.46+, and the [Tauri 2 platform prerequisites](https://v2.tauri.app/start/prerequisites/). Project CI uses Rust 1.99.1, and Cargo declares a Rust minimum of 1.99. The verified environment uses Node 24.18.0, Rust 1.99.1, and Git 2.55.0.windows.1.

```sh
npm ci
npm run desktop:dev
```

If the current Windows workspace already contains local Rust/MSVC/SDK tools under `.tools/`, first run `. ./scripts/dev-env.ps1` in PowerShell. These tools are not distributed with the repository; install the standard platform toolchain on other machines.

| Purpose                   | Command                                   | Behavior                                                                                       |
| ------------------------- | ----------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Browser layout preview    | `npm run dev`                             | Open `http://127.0.0.1:1420/`; edits normally update automatically. No local repository access |
| Desktop development       | `npm run desktop:dev`                     | Includes the Rust backend for reading repositories, scanning, and saving                       |
| Frontend checks and tests | `npm run check`                           | TypeScript checks and Vitest tests                                                             |
| Frontend production build | `npm run build`                           | Generates `dist/`; does not start a server or update an installed desktop application          |
| Windows installer         | `npm run desktop:build -- --bundles nsis` | Repackages the frontend and desktop application under `src-tauri/target/release/`              |

### Port conflicts or edits not appearing

- **`Port 1420 is already in use`:** A process already occupies the development port. If this project's server is running, visit its address directly. To restart it, press `Ctrl+C` in its original terminal and start it again.
- **Browser not updating:** Save your files, confirm that you are visiting `http://127.0.0.1:1420/`, then press `Ctrl+F5` to force a refresh. Check the development terminal for errors.
- **Installed or standalone Release application not updating:** It contains the frontend from build time. Use `npm run desktop:dev`, or rebuild and launch the new executable. Refreshing a browser cannot update a packaged application.
- **Repository lists or details unavailable in the browser:** The preview has no mock business data. Add real repositories in desktop development mode to inspect these areas.

## Verification

```sh
npm run check
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --features desktop --locked -- -D warnings
```

For the real Windows desktop regression flow, first build the Release executable, then run `node scripts/desktop-smoke.mjs` or `scripts/desktop-smoke.ps1` in an interactive Windows session. The script uses isolated Git configuration, sample repositories, and a separate WebView2 test directory to exercise real IPC. It closes test processes and removes sample entries afterward. Do not run multiple copies concurrently.

The review-fix baseline records 48 Rust tests, 10 frontend tests, and 10 desktop workflow stages. Later interface changes do not automatically inherit that desktop validation result. Windows has internal build validation; macOS, Linux, and independent installation/uninstallation remain pending. A CI configuration does not establish cross-platform validation. See the [verification record](docs/VERIFICATION.md) and [review-fix record](docs/REVIEW-FIXES.md) for the tested scope.

## Project structure and documentation

| Path                                          | Contents                                                                                                        |
| --------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| `src/App.tsx`                                 | React workbench and interactions                                                                                |
| `src/tokens.css` / `src/styles.css`           | Shared design tokens, component styles, and responsive layout                                                   |
| `src/api.ts` / `src/types.ts` / `src/view.ts` | Desktop IPC, types, and presentation rules                                                                      |
| `src-tauri/src/`                              | Git adapter, scanning, identity checks, authentication metadata, previewed writes, SQLite, and desktop commands |
| `src-tauri/tests/core.rs`                     | Isolated repository integration tests that do not modify personal Git configuration                             |
| `src-tauri/examples/benchmark.rs`             | Local performance fixtures for 10 / 100 / 500 entries                                                           |
| `docs/`                                       | Design, implementation, verification, and review-fix records                                                    |

- [Design document](docs/design-v1.0.0.md) (Chinese)
- [Implementation record](docs/IMPLEMENTATION.md) (Chinese)
- [Product overview](PRODUCT.md) (English)

## License

No open-source license has been selected. The files do not grant an open-source license. Installers are for internal validation and have not been formally released.
