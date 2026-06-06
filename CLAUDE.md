# wmux — Agent Instructions

## What is this?

wmux is a Windows-native terminal multiplexer for AI coding agents, inspired by cmux (macOS-only). It's a Tauri v2 app with a Rust backend and HTML/JS frontend.

## Architecture

```
src-tauri/           Rust backend (Tauri v2)
  src/
    main.rs          Entry point → calls wmux_lib::run()
    lib.rs           App setup, all Tauri commands registered
    workspace.rs     Workspace CRUD, agent tracking, git info
    pty_manager.rs   PTY spawn/read/write/resize + OSC parser
    notifications.rs Notification system + Windows Toast
    session.rs       Session save/restore to %LOCALAPPDATA%\wmux
    wave_bridge.rs   Wave AI filesystem/CLI/HTTP bridge
  Cargo.toml         Rust dependencies
  tauri.conf.json    Tauri config, window settings, CSP

ui/                  Frontend (plain HTML/JS, no build step)
  index.html         Main UI: sidebar, pane grid, status bar
  app.js             Tauri invoke bridge, state management, keyboard shortcuts
  panes.js           Split pane resize handles, focus navigation
```

## Build & Test

```bash
# Build check
cd src-tauri && cargo check

# Tests
cd src-tauri && cargo test

# Dev mode (requires npm install first)
npx tauri dev
```

## Conventions

- Follow the patterns in Archivist-Agent's src-tauri (same author)
- Tauri commands: snake_case, return Result<T, String>
- Global state: once_cell::Lazy<Mutex<HashMap<...>>>
- Frontend: vanilla JS, no framework (Phase 1), use window.__TAURI__.core.invoke()
- Every Rust module has #[cfg(test)] mod tests {}
- Commit messages: conventional commits with context

## Current Phase

Phase 1 complete: UI prototype + Tauri backend stubs + command palette + keyboard shortcuts

Phase 2 next: xterm.js terminal rendering + portable-pty backend

## Important Files

- `ui/app.js` — the main frontend logic, mock data for browser dev, Tauri bridge
- `src-tauri/src/workspace.rs` — core workspace model, agent status tracking
- `src-tauri/src/pty_manager.rs` — PTY management (stubs, ready for portable-pty)
- `WAVE_BRIDGE.md` — how the Wave AI integration works
- `ROADMAP.md` — full development plan
