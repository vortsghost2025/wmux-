# Wave AI ↔ wmux Bridge

## How It Works

Your existing Wave AI shell runs in one of wmux's terminal panes. The bridge gives Wave AI **direct programmatic access** to all your other terminals, workspaces, and agents.

```
┌─────────────────────────────────────────────────┐
│  Wave AI (your existing build)                  │
│  ┌────────────────────────────────────────────┐ │
│  │ "check all agent status"                   │ │
│  │                                            │ │
│  │ wave-bridge writes request →               │ │
│  │ S:\waveterm\bridge\req-abc123.json         │ │
│  └────────────────────────────────────────────┘ │
└────────────────────┬────────────────────────────┘
                     │ filesystem watch
                     ▼
┌─────────────────────────────────────────────────┐
│  wmux Bridge Service                            │
│  ┌────────────────────────────────────────────┐ │
│  │ Reads request → gathers terminal data →    │ │
│  │ writes response:                           │ │
│  │ S:\waveterm\bridge\res-abc123.json         │ │
│  │                                            │ │
│  │ {                                          │ │
│  │   "workspaces": [                          │ │
│  │     { "name": "kucoin-lane",               │ │
│  │       "agents": [                          │ │
│  │         { "name": "GLM-5.1",               │ │
│  │           "status": "waiting",             │ │
│  │           "last_output": "..." }           │ │
│  │       ]                                    │ │
│  │     }                                      │ │
│  │   ]                                        │ │
│  │ }                                          │ │
│  └────────────────────────────────────────────┘ │
└────────────────────┬────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────┐
│  Wave AI reads response, acts on it             │
│  "GLM-5.1 is waiting in kucoin-lane,            │
│   it found DEX module test failures.            │
│   Shall I send it direction?"                   │
└─────────────────────────────────────────────────┘
```

## Three Bridge Modes

### 1. Filesystem Bridge (Recommended Start)
Simplest to integrate with your existing Wave setup.

```powershell
# Start the bridge watcher
wmux wave bridge --watch "S:\waveterm\bridge"

# Wave AI can write requests as JSON files:
# S:\waveterm\bridge\req-{uuid}.json

# wmux watches, processes, writes responses:
# S:\waveterm\bridge\res-{uuid}.json
```

**Request format:**
```json
{
  "id": "req-abc123",
  "command": "scan_terminals",
  "params": {
    "workspace": "kucoin-lane",
    "lines": 50
  }
}
```

**Response format:**
```json
{
  "id": "req-abc123",
  "success": true,
  "data": {
    "terminals": [
      {
        "pane_id": "p1",
        "agent": "GLM-5.1",
        "status": "waiting",
        "last_lines": ["...", "..."]
      }
    ]
  }
}
```

### 2. CLI Bridge
For when Wave AI can run shell commands:

```powershell
# From Wave AI's shell:
wmux status --json                          # Get all workspace/agent status
wmux send --workspace kucoin-lane "yes"     # Send input to agent
wmux notify --title "Wave" --body "Done"    # Send notification
wmux wave ask "what are all agents doing?"  # Capture all terminal content
```

### 3. HTTP API Bridge
For more sophisticated integration:

```powershell
# wmux exposes a local HTTP API:
wmux wave api --port 9876

# Wave AI can call:
# GET  http://localhost:9876/workspaces
# GET  http://localhost:9876/agents
# POST http://localhost:9876/send { workspace, pane, text }
# GET  http://localhost:9876/notifications (SSE stream)
# POST http://localhost:9876/capture { workspace, lines }
```

## Wave Shell Command Integration

In your Wave AI shell, you can add a bridge command:

```javascript
// In your Wave AI source:
// When user says "check all terminals" or "scan agents"

const { execSync } = require('child_process');

function askWmux(query) {
  const result = execSync(`wmux wave ask "${query}" --json`, {
    encoding: 'utf-8'
  });
  return JSON.parse(result);
}

function sendToAgent(workspace, text) {
  execSync(`wmux send --workspace "${workspace}" "${text}"`);
}

// Usage in Wave AI conversation:
// User: "what are all my agents doing?"
// Wave: *calls askWmux("status")* → formats response
// User: "tell GLM to proceed with the fix"
// Wave: *calls sendToAgent("kucoin-lane", "proceed with the fix")*
```

## Agent Hook Setup

wmux installs hooks that agents use to report their status:

```powershell
# Install hooks for all detected agents
wmux hooks setup

# Install for specific agent
wmux hooks setup --agent claude
wmux hooks setup --agent kilo
wmux hooks setup --agent opencode

# The hooks write status to ~/.wmuxterm/
# wmux reads these to show agent status in the sidebar
```

## Notification Flow

```
Agent outputs "Waiting for input"
  → wmux detects via OSC parse or pattern match
  → Sidebar: blue ring on workspace
  → Tab: dot indicator changes to blue
  → Status bar: "1 waiting"
  → Windows Toast notification (if high urgency)
  → Wave AI bridge: notification event
  → Wave AI: "GLM-5.1 in kucoin-lane needs your input"
```
