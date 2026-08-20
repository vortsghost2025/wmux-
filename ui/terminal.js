// terminal.js — xterm.js terminal manager
// Works with the recursive split-pane system in panes.js

(function() {
const TAURI = window.__TAURI__;
const isTauri = !!TAURI;
const XTerm = window.Terminal;
const FitAddonClass = window.FitAddon?.FitAddon;
const WebglAddonClass = window.WebglAddon?.WebglAddon;
const WebLinksAddonClass = window.WebLinksAddon?.WebLinksAddon;

const terminals = {};
let globalPaneCounter = 0;

const WMUX_THEME = {
  background: '#0d1117',
  foreground: '#e6edf3',
  cursor: '#e6edf3',
  cursorAccent: '#0d1117',
  selectionBackground: 'rgba(88,166,255,0.3)',
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

// ===== Create a terminal in a container =====

async function createTerminal(containerId, cwd, workspaceId) {
  const container = document.getElementById(containerId);
  if (!container || !XTerm || !FitAddonClass) {
    console.error('[terminal] Missing container or xterm:', containerId);
    return null;
  }

  // Don't recreate if already exists
  if (terminals[containerId]) {
    return terminals[containerId];
  }

  const term = new XTerm({
    theme: WMUX_THEME,
    fontFamily: "'Cascadia Code', 'Cascadia Mono', 'Consolas', monospace",
    fontSize: 14,
    lineHeight: 1.25,
    cursorBlink: true,
    cursorStyle: 'block',
    scrollback: 10000,
    allowProposedApi: true,
    convertEol: true,
  });

  const fitAddon = new FitAddonClass();
  term.loadAddon(fitAddon);

  if (WebLinksAddonClass) {
    try { term.loadAddon(new WebLinksAddonClass()); } catch(e) {}
  }

  term.open(container);

  if (WebglAddonClass) {
    try { term.loadAddon(new WebglAddonClass()); } catch(e) {
      console.warn('[terminal] WebGL failed, using canvas');
    }
  }

  // Fit after a brief delay to let layout settle
  setTimeout(() => fitAddon.fit(), 50);
  setTimeout(() => fitAddon.fit(), 200);

  let ptyId = null;

  if (isTauri) {
    try {
      const info = await TAURI.core.invoke('create_pty', {
        cwd: cwd || 'C:\\\\Users\\\\seand',
        cols: term.cols,
        rows: term.rows,
        workspaceId: workspaceId || 'default',
      });
      ptyId = info.id;
      console.log(`[terminal] PTY ${ptyId} created (${term.cols}x${term.rows})`);
    } catch (e) {
      console.error('[terminal] create_pty failed:', e);
      term.writeln('\x1b[31mPTY failed: ' + e + '\x1b[0m');
    }
  } else {
    term.writeln('\x1b[38;2;88;166;255mwmux\x1b[0m — browser dev mode (no PTY)');
    term.writeln('Run \x1b[32mnpx tauri dev\x1b[0m for real terminals');
  }

  // Input → PTY
  term.onData((data) => {
    if (ptyId && isTauri) {
      TAURI.core.invoke('write_pty', { ptyId, data }).catch(() => {});
    }
  });

  // Auto-resize
  const ro = new ResizeObserver(() => {
    try {
      fitAddon.fit();
      if (ptyId && isTauri) {
        TAURI.core.invoke('resize_pty', {
          ptyId,
          cols: term.cols,
          rows: term.rows,
        }).catch(() => {});
      }
    } catch(e) {}
  });
  ro.observe(container);

  const entry = { term, fitAddon, ptyId, ro, containerId, cwd: cwd || null };
  terminals[containerId] = entry;
  return entry;
}

// ===== PTY data listener =====

function setupPtyListener() {
  if (!isTauri) return;

  TAURI.event.listen('pty_data', (event) => {
    const { pty_id, data } = event.payload;
    const entry = Object.values(terminals).find(t => t.ptyId === pty_id);
    if (entry) entry.term.write(data);
  });

  TAURI.event.listen('pty_exit', (event) => {
    const { pty_id, code } = event.payload;
    const entry = Object.values(terminals).find(t => t.ptyId === pty_id);
    if (entry) {
      entry.term.writeln(`\r\n\x1b[33m[exited: ${code}]\x1b[0m`);
      entry.ptyId = null;
    }
  });

  TAURI.event.listen('agent_waiting', (event) => {
    const { pty_id, workspace_id, title, body } = event.payload;
    if (window.showToast) window.showToast(title, body, 'normal');
    if (window.__wmux_incrementUnread) window.__wmux_incrementUnread(workspace_id);
    for (const [containerId, entry] of Object.entries(terminals)) {
      if (entry.ptyId === pty_id) {
        const pane = document.getElementById(containerId)?.closest('.pane-leaf');
        if (pane) pane.classList.add('waiting');
        break;
      }
    }
  });
}

// ===== Close terminal =====

function closeTerminal(containerId) {
  const entry = terminals[containerId];
  if (!entry) return;
  if (entry.ptyId && isTauri) {
    TAURI.core.invoke('close_pty', { ptyId: entry.ptyId }).catch(() => {});
  }
  entry.ro?.disconnect();
  entry.term?.dispose();
  delete terminals[containerId];
}

// ===== Expose globally =====

window.terminals = terminals;
window.createTerminal = createTerminal;
window.closeTerminal = closeTerminal;

// ===== Init =====

window.addEventListener('DOMContentLoaded', () => {
  if (!XTerm) {
    console.error('[terminal] xterm.js not loaded!');
    return;
  }
  console.log('[terminal] xterm.js ready, initializing PaneManager...');
  setupPtyListener();

  // Give PaneManager a moment to init, then create the root leaf
  setTimeout(() => {
    if (window.PaneManager && !window.PaneManager.root) {
      window.PaneManager.createRootLeaf('xterm-initial', 'terminal');
    }
  }, 100);
});

})();
