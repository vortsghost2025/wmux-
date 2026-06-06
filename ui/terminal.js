// terminal.js — xterm.js terminal manager
// Creates real terminal instances in pane divs, bridges to Tauri PTY backend
//
// IPC Contract with pty_manager.rs:
//   Events (Rust → JS):  pty_data { ptyId, data }  |  pty_exit { ptyId, code }
//   Commands (JS → Rust): create_pty, write_pty, resize_pty, close_pty

const TAURI = window.__TAURI__;
const isTauri = !!TAURI;

// CDN-loaded xterm.js (loaded via index.html script tags)
// In Phase 3, bundle locally to avoid CDN dependency
const XTerm = window.Terminal;
const FitAddon = window.FitAddon?.FitAddon;
const WebglAddon = window.WebglAddon?.WebglAddon;
const WebLinksAddon = window.WebLinksAddon?.WebLinksAddon;

// ===== Terminal Registry =====

const terminals = {};  // paneId -> { term, fitAddon, ptyId, element }

// ===== Theme =====

const WMUX_THEME = {
  background: '#0d1117',
  foreground: '#e6edf3',
  cursor: '#e6edf3',
  cursorAccent: '#0d1117',
  selectionBackground: 'rgba(88,166,255,0.3)',
  selectionForeground: '#ffffff',
  black: '#484f58',
  red: '#f85149',
  green: '#3fb950',
  yellow: '#d29922',
  blue: '#58a6ff',
  magenta: '#bc8cff',
  cyan: '#39d353',
  white: '#e6edf3',
  brightBlack: '#6e7681',
  brightRed: '#ffa198',
  brightGreen: '#56d364',
  brightYellow: '#e3b341',
  brightBlue: '#79c0ff',
  brightMagenta: '#d2a8ff',
  brightCyan: '#56d364',
  brightWhite: '#ffffff',
};

// ===== Create Terminal =====

async function createTerminal(paneId, cwd) {
  const container = document.getElementById(paneId);
  if (!container) {
    console.error(`[terminal] Pane ${paneId} not found`);
    return null;
  }

  // Find or create the terminal div inside the pane
  let termDiv = container.querySelector('.xterm-container');
  if (!termDiv) {
    // Clear the static mock terminal content
    const oldTerminal = container.querySelector('.terminal');
    if (oldTerminal) {
      termDiv = document.createElement('div');
      termDiv.className = 'xterm-container';
      termDiv.style.cssText = 'flex:1; overflow:hidden;';
      oldTerminal.replaceWith(termDiv);
    } else {
      termDiv = document.createElement('div');
      termDiv.className = 'xterm-container';
      termDiv.style.cssText = 'flex:1; overflow:hidden;';
      container.appendChild(termDiv);
    }
  }

  // Create xterm.js instance
  const term = new XTerm({
    theme: WMUX_THEME,
    fontFamily: "'Cascadia Code', 'Cascadia Mono', 'Consolas', 'JetBrains Mono', monospace",
    fontSize: 13,
    lineHeight: 1.3,
    cursorBlink: true,
    cursorStyle: 'block',
    scrollback: 10000,
    allowProposedApi: true,
    convertEol: true,
  });

  // Fit addon — auto-resize terminal to container
  const fitAddon = new FitAddon();
  term.loadAddon(fitAddon);

  // Web links addon — clickable URLs
  if (WebLinksAddon) {
    term.loadAddon(new WebLinksAddon());
  }

  // Open terminal in the container
  term.open(termDiv);

  // Try WebGL addon for GPU-accelerated rendering
  if (WebglAddon) {
    try {
      term.loadAddon(new WebglAddon());
    } catch (e) {
      console.warn('[terminal] WebGL addon failed, falling back to canvas:', e);
    }
  }

  // Fit to container
  fitAddon.fit();

  const cols = term.cols;
  const rows = term.rows;

  let ptyId = null;

  // Create PTY on the backend
  if (isTauri) {
    try {
      const ptyInfo = await TAURI.core.invoke('create_pty', {
        cwd: cwd || 'C:\\Users\\' + (await getUsername()),
        cols,
        rows,
      });
      ptyId = ptyInfo.id;
      console.log(`[terminal] PTY created: ${ptyId} (${cols}x${rows})`);
    } catch (e) {
      console.error('[terminal] Failed to create PTY:', e);
      term.writeln('\x1b[31mFailed to create PTY: ' + e + '\x1b[0m');
      term.writeln('\x1b[33mRunning in demo mode — PTY backend not yet wired.\x1b[0m');
      writeDemoContent(term, paneId);
    }
  } else {
    // Browser dev mode — show demo content
    writeDemoContent(term, paneId);
  }

  // Wire terminal input → PTY write
  term.onData((data) => {
    if (ptyId && isTauri) {
      TAURI.core.invoke('write_pty', { ptyId, data }).catch(e => {
        console.error('[terminal] write_pty failed:', e);
      });
    }
  });

  // Handle resize
  const resizeObserver = new ResizeObserver(() => {
    fitAddon.fit();
    if (ptyId && isTauri) {
      TAURI.core.invoke('resize_pty', {
        ptyId,
        cols: term.cols,
        rows: term.rows,
      }).catch(e => console.warn('[terminal] resize failed:', e));
    }
  });
  resizeObserver.observe(termDiv);

  // Store in registry
  terminals[paneId] = { term, fitAddon, ptyId, element: termDiv, resizeObserver };

  return terminals[paneId];
}

// ===== Listen for PTY data events =====

function setupPtyDataListener() {
  if (!isTauri) return;

  TAURI.event.listen('pty_data', (event) => {
    const { ptyId, data } = event.payload;
    // Find the terminal with this ptyId
    const entry = Object.values(terminals).find(t => t.ptyId === ptyId);
    if (entry) {
      // data is a Uint8Array or base64 string — write to terminal
      if (typeof data === 'string') {
        entry.term.write(data);
      } else {
        entry.term.write(new Uint8Array(data));
      }
    }
  });

  TAURI.event.listen('pty_exit', (event) => {
    const { ptyId, code } = event.payload;
    const entry = Object.values(terminals).find(t => t.ptyId === ptyId);
    if (entry) {
      entry.term.writeln(`\r\n\x1b[33m[Process exited with code ${code}]\x1b[0m`);
      entry.ptyId = null;
    }
  });
}

// ===== Demo content for browser dev / when PTY isn't wired =====

function writeDemoContent(term, paneId) {
  term.writeln('');
  term.writeln('\x1b[38;2;88;166;255m  ╔══════════════════════════════════════╗\x1b[0m');
  term.writeln('\x1b[38;2;88;166;255m  ║\x1b[0m  \x1b[1;37mwmux\x1b[0m v0.1.0 — Terminal Pane         \x1b[38;2;88;166;255m║\x1b[0m');
  term.writeln('\x1b[38;2;88;166;255m  ║\x1b[0m                                      \x1b[38;2;88;166;255m║\x1b[0m');
  term.writeln('\x1b[38;2;88;166;255m  ║\x1b[0m  \x1b[33mPhase 2:\x1b[0m xterm.js loaded ✓           \x1b[38;2;88;166;255m║\x1b[0m');
  term.writeln('\x1b[38;2;88;166;255m  ║\x1b[0m  \x1b[33mNext:\x1b[0m    portable-pty backend         \x1b[38;2;88;166;255m║\x1b[0m');
  term.writeln('\x1b[38;2;88;166;255m  ║\x1b[0m                                      \x1b[38;2;88;166;255m║\x1b[0m');
  term.writeln('\x1b[38;2;88;166;255m  ║\x1b[0m  \x1b[32mCtrl+Shift+P\x1b[0m  command palette       \x1b[38;2;88;166;255m║\x1b[0m');
  term.writeln('\x1b[38;2;88;166;255m  ║\x1b[0m  \x1b[32mCtrl+B\x1b[0m        toggle sidebar         \x1b[38;2;88;166;255m║\x1b[0m');
  term.writeln('\x1b[38;2;88;166;255m  ║\x1b[0m  \x1b[32mCtrl+D\x1b[0m        split right            \x1b[38;2;88;166;255m║\x1b[0m');
  term.writeln('\x1b[38;2;88;166;255m  ║\x1b[0m  \x1b[32mAlt+Arrow\x1b[0m     focus pane             \x1b[38;2;88;166;255m║\x1b[0m');
  term.writeln('\x1b[38;2;88;166;255m  ╚══════════════════════════════════════╝\x1b[0m');
  term.writeln('');

  if (paneId === 'pane-1') {
    // Simulate agent-like output
    term.writeln('\x1b[38;2;188;140;255m[GLM-5.1]\x1b[0m Loading workspace...');
    term.writeln('\x1b[38;2;188;140;255m[GLM-5.1]\x1b[0m Scanning S:\\wmux-\\src-tauri\\src\\');
    term.writeln('\x1b[38;2;188;140;255m[GLM-5.1]\x1b[0m Found 7 Rust modules');
    term.writeln('');
    term.writeln('\x1b[38;2;57;211;83m❯\x1b[0m \x1b[2mType here when PTY is connected...\x1b[0m');
  } else if (paneId === 'pane-2') {
    term.writeln('\x1b[38;2;63;185;80m[Kilo]\x1b[0m Ready.');
    term.writeln('\x1b[32m$\x1b[0m \x1b[2mWaiting for PTY backend...\x1b[0m');
  } else if (paneId === 'pane-3') {
    term.writeln('\x1b[38;2;88;166;255m[Wave AI]\x1b[0m Bridge ready on port 51987');
    term.writeln('\x1b[32m❯\x1b[0m \x1b[2mPTY not yet connected\x1b[0m');
  }
}

async function getUsername() {
  try {
    if (isTauri) {
      // Try to get username from environment
      return 'seand'; // fallback, Phase 2 will read from env
    }
  } catch (e) {}
  return 'user';
}

// ===== Destroy Terminal =====

function destroyTerminal(paneId) {
  const entry = terminals[paneId];
  if (!entry) return;

  if (entry.ptyId && isTauri) {
    TAURI.core.invoke('close_pty', { ptyId: entry.ptyId }).catch(() => {});
  }
  entry.resizeObserver?.disconnect();
  entry.term.dispose();
  delete terminals[paneId];
}

// ===== Init =====

window.addEventListener('DOMContentLoaded', () => {
  // Wait a tick for the DOM to settle
  setTimeout(() => {
    // Check if xterm.js loaded
    if (!XTerm) {
      console.warn('[terminal] xterm.js not loaded — keeping static terminals');
      return;
    }

    console.log('[terminal] xterm.js loaded, creating terminals...');

    // Create terminals for each pane
    createTerminal('pane-1', 'S:\\wmux-');
    createTerminal('pane-2', 'S:\\wmux-');
    createTerminal('pane-3', 'S:\\wmux-');

    // Setup PTY data listener
    setupPtyDataListener();
  }, 100);
});

// Export
window.createTerminal = createTerminal;
window.destroyTerminal = destroyTerminal;
window.terminals = terminals;
