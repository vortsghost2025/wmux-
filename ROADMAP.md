# wmux Development Roadmap

## Phase 1: Core Terminal (Week 1-2)
- [x] Project scaffold (Tauri v2 + Rust)
- [x] UI prototype (HTML/CSS mockup)
- [x] Architecture design
- [ ] **xterm.js integration** — terminal rendering in WebView2
- [ ] **conpty PTY backend** — spawn PowerShell/cmd via Windows conpty API
- [ ] **PTY ↔ xterm.js bridge** — Tauri events for bidirectional data flow
- [ ] Basic split pane layout (horizontal + vertical)
- [ ] Workspace model (create, switch, close)

## Phase 2: Sidebar & Workspaces (Week 2-3)
- [ ] Vertical sidebar with workspace list
- [ ] Git branch detection per workspace
- [ ] Listening port detection (`netstat` parsing)
- [ ] Workspace persistence (save/load)
- [ ] Tab bar within workspaces (surfaces)
- [ ] Keyboard shortcuts (Ctrl+1-9, Ctrl+T, Ctrl+D, etc.)

## Phase 3: Agent Awareness (Week 3-4)
- [ ] **OSC sequence parser** — detect notifications from terminal output
- [ ] **Agent detection heuristics** — recognize Claude, Kilo, GLM, Copilot patterns
- [ ] Blue ring effect on waiting agents
- [ ] Unread counter per workspace
- [ ] `wmux notify` CLI command
- [ ] Windows Toast notifications for high-urgency
- [ ] Jump to latest unread (Ctrl+Shift+U)

## Phase 4: CLI & IPC (Week 4-5)
- [ ] Named pipe IPC server
- [ ] `wmux` CLI binary
- [ ] Full CLI command set (workspace, split, send, notify, browser, etc.)
- [ ] Agent hook installer (`wmux hooks setup`)

## Phase 5: Wave AI Bridge (Week 5-6)
- [ ] Filesystem bridge (watch directory for request/response files)
- [ ] HTTP API bridge (localhost REST API)
- [ ] Terminal content capture (scrollback extraction)
- [ ] "Ask all terminals" command
- [ ] Auto-forward notifications to Wave

## Phase 6: Browser & Polish (Week 6-8)
- [ ] In-app browser pane (WebView2)
- [ ] Browser address bar and navigation
- [ ] Session save/restore
- [ ] Agent session resume
- [ ] Theme engine (import Windows Terminal / Ghostty themes)
- [ ] Settings UI
- [ ] Command palette (Ctrl+Shift+P)

## Phase 7: Release (Week 8+)
- [ ] Windows installer (MSI/MSIX)
- [ ] Auto-update via GitHub releases
- [ ] Documentation site
- [ ] GitHub Actions CI/CD

## Technical Decisions

### Why Tauri v2 over Electron?
- 10-50x smaller binary (~5MB vs ~150MB+)
- Uses system WebView2 (already on Win 11)
- Rust backend = native performance for PTY management
- You already have Rust/Cargo in your toolchain

### Why xterm.js over native terminal rendering?
- Battle-tested terminal emulator (used by VS Code, Hyper, etc.)
- WebGL addon for GPU-accelerated rendering
- Fits naturally into Tauri's WebView2
- Rich addon ecosystem (fit, search, web-links, image)

### Why not just fork cmux?
- cmux is 83.5% Swift with macOS-only frameworks (AppKit, SwiftUI)
- It embeds libghostty which has no Windows support
- Porting would mean rewriting ~90% of the code anyway
- Clean-room implementation lets us add Windows-native features
  (named pipes, conpty, Windows Terminal theme import, Wave bridge)

### Why not Windows Terminal + tmux?
- tmux doesn't have agent-aware notifications
- No blue ring / unread tracking for AI agents
- No in-app browser
- No CLI for programmatic control from Wave AI
- No vertical sidebar with workspace metadata
- wmux gives you all of that in one app
