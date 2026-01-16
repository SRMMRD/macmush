# MACMush

A modern MUD/MUSH client for macOS built with Tauri 2.x, Rust, and JavaScript.

## Download

Download the latest release from the [Releases](https://github.com/SRMMRD/macmush/releases) page:

- **MACMush_0.1.0_universal.dmg** - Universal binary for Intel and Apple Silicon Macs

### System Requirements

- macOS 12.0 (Monterey) or later
- Intel or Apple Silicon Mac
- 50MB free disk space

### Installation

1. Download the DMG file
2. Open the DMG and drag **MACMush** to your Applications folder
3. First launch: Right-click MACMush.app and select "Open" (required for unsigned apps)
4. Subsequent launches work normally from Applications or Spotlight

## Features

- **Full Automation Support**: Triggers, Aliases, Timers, and Highlights
- **Real-time Session Sync**: Automation changes apply instantly without reconnect
- **ANSI/Telnet Support**: Full color support and telnet protocol handling
- **MCCP Compression**: Mud Client Compression Protocol support
- **Speedwalk**: Quick navigation with directional commands (e.g., "4n2e")
- **Command History**: Track and recall previous commands
- **Tab Completion**: Auto-complete commands and navigation
- **World Management**: Save and manage multiple MUD/MUSH connections

## Getting Started

1. **Create a World**: Click "New World" and enter your MUD's hostname and port
2. **Connect**: Select your world and click "Connect"
3. **Automate**: Use the Triggers, Aliases, Timers, and Highlights panels to set up automation
4. **Save**: All automation and worlds are automatically saved

### Quick Examples

**Alias**: Create shortcuts for common commands
- Pattern: `^gg$` → Command: `get all from corpse`

**Trigger**: Auto-respond to game events
- Pattern: `You are hungry` → Command: `eat bread`

**Timer**: Execute commands on a schedule
- Interval: `60000` (1 minute) → Command: `save`

**Highlight**: Color important text
- Pattern: `tells you` → Color: `yellow`

## Architecture

### Backend (Rust)
- **Core**: Connection management, session state, event bus
- **Automation**: Trigger/Alias/Timer/Highlight managers with regex matching
- **Network**: TCP with TLS support, telnet/ANSI parsing, MCCP decompression
- **Scripting**: Lua 5.4 integration for advanced automation

### Frontend (JavaScript)
- **UI**: Vanilla JavaScript with xterm.js terminal emulation
- **Event-driven**: Listens for backend events via Tauri IPC
- **Persistence**: JSON-based storage for worlds and automation

## Building from Source

### Prerequisites
- Rust 1.75+
- Node.js 18+
- macOS 12+

### Development
```bash
npm install
npm run dev
```

### Production Build
```bash
npm run build
```

The DMG installer will be created at:
```
src-tauri/target/release/bundle/dmg/MACMush_0.1.0_universal.dmg
```

## Project Structure

```
├── src/                    # Frontend (HTML/CSS/JS)
│   ├── main.js            # Main application logic
│   ├── ansi-parser.js     # ANSI color parsing
│   ├── styles.css         # Application styles
│   └── index.html         # Main HTML
├── src-tauri/             # Backend (Rust)
│   ├── src/
│   │   ├── core/          # Session, connection, event bus
│   │   ├── automation/    # Triggers, aliases, timers, highlights
│   │   ├── network/       # TCP, telnet, MCCP, MXP
│   │   └── ui/            # Tauri commands and event forwarding
│   └── Cargo.toml
└── package.json
```

## MVP Status

✅ **COMPLETE** - All core features implemented and tested:
- Connection lifecycle (connect/disconnect/reconnect)
- User input and command execution with speedwalk/alias expansion
- Trigger system with matching and execution
- Alias system with pattern matching and command expansion
- Timer system with scheduling and stop-on-disconnect
- Highlight system with style application
- Event-driven architecture (Backend → EventBus → Frontend)
- Data flow (MUD server → Processing → Display)
- Persistence (JSON file storage for all automation types)
- Real-time session sync (no reconnect needed for changes)

## License

MACMush is licensed under the MIT License. See [LICENSE](LICENSE) for details.

Copyright © 2025 Scott Mitchell

## Author

**Scott Mitchell**

## Contributing

Contributions are welcome! Feel free to:
- Report bugs and request features via [GitHub Issues](https://github.com/SRMMRD/macmush/issues)
- Submit pull requests
- Improve documentation
- Share your automation scripts
