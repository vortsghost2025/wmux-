# wmux — cmux-style AI Agent Terminal for Windows

> A Windows-native terminal multiplexer with vertical sidebar tabs, agent notifications, split panes, and a scriptable CLI — inspired by [cmux](https://github.com/manaflow-ai/cmux).

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  wmux (Tauri v2 App)                                            │
│  ┌──────────┬──────────────────────────────────────────────────┐│
│  │ Sidebar  │  Terminal Pane Grid                              ││
│  │          │  ┌─────────────────┬─────────────────────────┐  ││
│  │ ● ws-1   │  │  xterm.js PTY   │  xterm.js PTY           │  ││
│  │   main   │  │  (agent: claude) │  (agent: kilo)          │  ││
│  │   🔵     │  │                  │                          │  ││
│  │          │  ├─────────────────┴─────────────────────────┤  ││
│  │ ○ ws-2   │  │  xterm.js PTY                              │  ││
│  │   kucoin │  │  (cargo build)                              │  ││
│  │          │  │                                              │  ││
│  │ ○ ws-3   │  └──────────────────────────────────────────────┘  ││
│  │   archi  │                                                    ││
│  │          │  ┌──────────────────────────────────────────────┐  ││
│  │          │  │  In-App Browser (WebView2)                   │  ││
│  │          │  │  localhost:3000                               │  ││
│  │          │  └──────────────────────────────────────────────┘  ││
│  └──────────┴────────────────────────────────────────────────────┘│
│  Status: 4 agents │ 2 waiting │ ws-1: kucoin-lane/main           │
└─────────────────────────────────────────────────────────────────┘

Backend (Rust / Tauri):
  ├── PTY management (windows-rs / conpty)
  ├── Socket API server (named pipe or TCP)
  ├── Agent hook system (OSC 9/99/777 + wmux notify)
  ├── Session save/restore
  └── CLI (wmux.exe)

Frontend (TypeScript / Solid or React):
  ├── xterm.js terminals
  ├── Sidebar with workspace tabs
  ├── Split pane manager
  ├── Notification system
  └── Theme engine (reads Ghostty/WT configs)
```

## Stack

| Layer | Technology | Why |
|-------|-----------|-----|
| App Shell | Tauri v2 | Native Windows, tiny footprint, Rust backend, WebView2 |
| Terminal Rendering | xterm.js + WebGL addon | Fast GPU-accelerated terminal in browser context |
| PTY Backend | windows-rs conpty | Native Windows pseudo-terminal API |
| UI Framework | SolidJS or React | Reactive UI for sidebar/splits/notifications |
| IPC | Tauri commands + events | Bidirectional Rust ↔ Frontend |
| CLI | Clap (Rust) | `wmux` CLI talks to running app via named pipe |
| Notifications | Windows Toast + in-app | Agent waiting → blue ring + system notification |
| Browser | WebView2 (built into Tauri) | In-app browser pane for dev server inspection |

## Feature Parity with cmux

| cmux Feature | wmux Status | Notes |
|-------------|-------------|-------|
| Vertical sidebar tabs | ✅ Planned | Workspaces with git branch, PR, ports |
| Split panes | ✅ Planned | Horizontal + vertical splits |
| Agent notifications | ✅ Planned | OSC sequences + `wmux notify` CLI |
| Blue ring / unread | ✅ Planned | Per-workspace attention indicators |
| Session restore | ✅ Planned | Save/restore layout + agent resume |
| In-app browser | ✅ Planned | WebView2 split pane |
| CLI control | ✅ Planned | `wmux` commands over named pipe |
| Ghostty config compat | 🔄 Partial | Theme/font import from ghostty config |
| Windows Terminal themes | ✅ Planned | Import WT color schemes |
| Wave AI bridge | ✅ Planned | Hook into your existing Wave setup |

## Quick Start (Development)

```powershell
# Prerequisites
# - Rust toolchain (you already have this)
# - Node.js 20+
# - WebView2 Runtime (pre-installed on Win 11)

git clone https://github.com/vortsghost2025/wmux
cd wmux

# Install JS deps
npm install

# Dev mode
npm run tauri dev

# Build release
npm run tauri build
```

## CLI Usage

```powershell
# Create workspace
wmux workspace new --name "kucoin-lane" --dir "C:\repos\kucoin-lane"

# Split pane
wmux split right
wmux split down

# Send command to pane
wmux send --pane 2 "cargo build"

# Notify from agent hook
wmux notify --title "Claude waiting" --body "kucoin-lane: needs input" --urgency high

# List workspaces
wmux ls

# Jump to unread
wmux jump-unread

# Open browser in split
wmux browser open http://localhost:3000
```

## Wave AI Integration

wmux can bridge with your existing Wave AI setup:

```powershell
# Wave AI writes a request → wmux reads it → acts → writes response
wmux wave-bridge --watch "S:\waveterm\requests"

# Or pipe directly
wave-ai "check the build" | wmux exec --workspace kucoin-lane
```

## License

GPL-3.0-or-later (matching cmux)
