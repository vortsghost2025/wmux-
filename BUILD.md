# Building wmux on Windows

## Prerequisites (you already have these)

1. **Rust toolchain** — `rustup` with stable channel
2. **Node.js 20+** — for the Tauri CLI
3. **WebView2 Runtime** — pre-installed on Windows 11

## First Build

```powershell
# Clone the repo
git clone https://github.com/vortsghost2025/wmux-.git
cd wmux-

# Install Tauri CLI
npm install

# Dev mode (hot-reload, opens window)
npx tauri dev

# Release build (creates installer in src-tauri/target/release/bundle/)
npx tauri build
```

## Dev Workflow

```powershell
# After pulling changes:
git pull
npx tauri dev
```

The UI (ui/index.html + ui/app.js) hot-reloads in dev mode.
Rust changes require a recompile (~10s incremental).

## Testing the Rust Backend

```powershell
cd src-tauri
cargo test
```

## What You'll See

On first launch, the app shows:
- Left sidebar with your workspaces (mock data matching your repos)
- Split terminal panes with agent badges
- Blue glowing ring on the "kucoin-lane" workspace (agent waiting)
- Notification toast
- Command palette via Ctrl+Shift+P

## Phase 2: Live Agent Test

When you're ready to test with a real agent:

```powershell
# 1. Clone kucoin-lane into the wmux directory
cd C:\repos
git clone https://github.com/vortsghost2025/kucoin-lane.git

# 2. Launch wmux
cd C:\repos\wmux-
npx tauri dev

# 3. In a separate terminal, run an agent
cd C:\repos\kucoin-lane
kilo .     # or: opencode .
```

The agent will show up in wmux's workspace sidebar once we wire
the PTY layer (Phase 2 — portable-pty + xterm.js).
