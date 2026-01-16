const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
import { parseAnsi, styleToCSS } from './ansi-parser.js';

// UI Elements
let connectionDialog;
let clientInterface;
let statusIndicator;
let statusText;
let outputDisplay;
let commandInput;
let connectForm;
let commandForm;
let disconnectBtn;
let connectError;
let triggerModal;
let triggerForm;
let triggerList;
let addTriggerBtn;
let closeTriggerBtn;
let cancelTriggerBtn;
let triggerError;
let testTriggerModal;
let testTriggerForm;
let closeTestModalBtn;
let cancelTestBtn;
let testResult;
let savedWorldsList;
let saveWorldBtn;
let recentConnectionsList;
let clearRecentBtn;
let aliasModal;
let aliasForm;
let aliasList;
let addAliasBtn;
let closeAliasModalBtn;
let cancelAliasBtn;
let aliasError;
let testAliasModal;
let testAliasForm;
let closeTestAliasModalBtn;
let cancelTestAliasBtn;
let testAliasResult;
let macroModal;
let macroForm;
let macroList;
let addMacroBtn;
let closeMacroModalBtn;
let cancelMacroBtn;
let macroError;
let macroKeyInput;
let highlightModal;
let highlightForm;
let highlightList;
let addHighlightBtn;
let closeHighlightModalBtn;
let cancelHighlightBtn;
let highlightError;
let variablesPanel;
let variablesList;
let worldsTab;
let triggersTab;
let aliasesTab;
let macrosTab;
let highlightsTab;
let worldsContent;
let triggersContent;
let aliasesContent;
let macrosContent;
let highlightsContent;
let clearOutputBtn;
let toggleLoggingBtn;
let openLogsBtn;
let searchOutputInput;
let searchPrevBtn;
let searchNextBtn;
let clearSearchBtn;
let scrollToBottomBtn;
let filterMudCheckbox;
let filterSystemCheckbox;
let filterCommandCheckbox;
let filterErrorCheckbox;
let statusBar;
let statusUptime;
let statusBytesSent;
let statusBytesReceived;
let statusVariables;
let scriptModal;
let scriptForm;
let scriptList;
let addScriptBtn;
let closeScriptModalBtn;
let cancelScriptBtn;
let scriptError;

// ===========================
// Multi-World State Management
// ===========================

/**
 * World object representing a single MUD connection
 */
class World {
  constructor(id, name, host, port, useTls = false) {
    this.id = id;
    this.name = name;
    this.host = host;
    this.port = port;
    this.useTls = useTls;

    // Connection state
    this.isConnected = false;
    this.connectionStartTime = null;
    this.bytesSent = 0;
    this.bytesReceived = 0;

    // Output buffer
    this.outputBuffer = [];
    this.outputFilters = { system: true, command: true, error: true, mud: true };
    this.isAutoScrollEnabled = true;
    this.searchMatches = [];
    this.currentSearchIndex = -1;

    // Command history
    this.commandHistory = [];
    this.commandHistoryIndex = -1;
    this.maxCommandHistory = 100;
    this.currentCommand = '';
    this.isSearchingHistory = false;
    this.historySearchQuery = '';
    this.historySearchMatches = [];
    this.historySearchIndex = -1;

    // Tab-completion
    this.tabCompletionMatches = [];
    this.tabCompletionIndex = -1;
    this.tabCompletionPartial = '';
    this.isTabCompleting = false;

    // Automation
    this.triggers = [];
    this.aliases = [];
    this.macros = {};
    this.highlights = [];
    this.scripts = [];
    this.timers = [];
    this.activeTimerIntervals = new Map();
    this.timerNextExecution = new Map();
    this.worlds = [];

    // Variables
    this.variables = {};
    this.displayedVariables = ['hp_current', 'hp_max', 'mana_current', 'mana_max'];

    // Logging
    this.isLogging = false;
    this.currentLogFile = null;
    this.logFormat = 'plain';
    this.logFilters = { system: true, command: true, error: true, mud: true };

    // Settings
    this.maxScrollbackLines = 5000;
    this.speedWalkKeys = true;
    this.maxLogSizeMB = 10;
    this.autoRotateLogs = true;
    this.keepAliveInterval = null;
    this.autoReconnectEnabled = false;
  }

  /**
   * Serialize world to JSON for storage
   */
  toJSON() {
    return {
      id: this.id,
      name: this.name,
      host: this.host,
      port: this.port,
      useTls: this.useTls,
      triggers: this.triggers,
      aliases: this.aliases,
      macros: this.macros,
      highlights: this.highlights,
      scripts: this.scripts,
      timers: this.timers,
      variables: this.variables,
      displayedVariables: this.displayedVariables,
      maxScrollbackLines: this.maxScrollbackLines,
      speedWalkKeys: this.speedWalkKeys,
      autoReconnectEnabled: this.autoReconnectEnabled
    };
  }

  /**
   * Load world from JSON
   */
  static fromJSON(data) {
    const world = new World(data.id, data.name, data.host, data.port, data.useTls || false);
    world.triggers = data.triggers || [];
    world.aliases = data.aliases || [];
    world.macros = data.macros || {};
    world.highlights = data.highlights || [];
    world.scripts = data.scripts || [];
    world.timers = data.timers || [];
    world.variables = data.variables || {};
    world.displayedVariables = data.displayedVariables || ['hp_current', 'hp_max', 'mana_current', 'mana_max'];
    world.maxScrollbackLines = data.maxScrollbackLines || 5000;
    world.speedWalkKeys = data.speedWalkKeys !== undefined ? data.speedWalkKeys : true;
    world.autoReconnectEnabled = data.autoReconnectEnabled || false;
    return world;
  }
}

// Multi-world state
let worlds = []; // Array of World objects
let activeWorldId = null; // Currently active world ID
let pendingHighlights = []; // Pending highlight matches from backend

// UI editing state (shared across worlds)
let editingTriggerIndex = null;
let testingTriggerIndex = null;
let editingAliasIndex = null;
let testingAliasIndex = null;
let editingMacroKey = null;
let editingHighlightIndex = null;
let editingScriptIndex = null;
let editingTimerIndex = null;

// Global state (not per-world)
let savedWorlds = []; // Saved connection profiles
let recentConnections = []; // Recent connection history
let statusBarUpdateInterval = null;

// ===========================
// World Management Functions
// ===========================

/**
 * Get the currently active world
 */
function getActiveWorld() {
  if (!activeWorldId) return null;
  return worlds.find(w => w.id === activeWorldId) || null;
}

/**
 * Create a new world
 */
function createWorld(name, host, port, useTls = false) {
  const id = 'world-' + Date.now() + '-' + Math.random().toString(36).substr(2, 9);
  const world = new World(id, name, host, port, useTls);
  worlds.push(world);
  return world;
}

/**
 * Switch to a different world
 */
function switchToWorld(worldId) {
  const world = worlds.find(w => w.id === worldId);
  if (!world) {
    console.error(`World ${worldId} not found`);
    return false;
  }

  activeWorldId = worldId;

  // Update UI to reflect the active world
  renderWorldTabs();
  renderOutputBuffer();
  updateStatusBar();
  renderTriggersList();
  renderAliasesList();
  renderMacrosList();
  renderHighlightsList();
  renderScriptsList();
  renderTimersList();
  renderVariablesList();

  console.log(`Switched to world: ${world.name}`);
  return true;
}

/**
 * Close a world and clean up its resources
 */
async function closeWorld(worldId) {
  const worldIndex = worlds.findIndex(w => w.id === worldId);
  if (worldIndex === -1) {
    console.error(`World ${worldId} not found`);
    return;
  }

  const world = worlds[worldIndex];

  // Disconnect if connected
  if (world.isConnected) {
    try {
      await invoke('disconnect', { worldId });
    } catch (error) {
      console.error('Error disconnecting world:', error);
    }
  }

  // Stop all timers
  for (const [timerId, intervalHandle] of world.activeTimerIntervals) {
    clearInterval(intervalHandle);
  }
  world.activeTimerIntervals.clear();
  world.timerNextExecution.clear();

  // Clear keep-alive if set
  if (world.keepAliveInterval) {
    clearInterval(world.keepAliveInterval);
  }

  // Remove from worlds array
  worlds.splice(worldIndex, 1);

  // If this was the active world, switch to another or clear
  if (activeWorldId === worldId) {
    if (worlds.length > 0) {
      switchToWorld(worlds[0].id);
    } else {
      activeWorldId = null;
      renderWorldTabs();
      outputDisplay.innerHTML = '<div class="empty-output">No worlds open. Connect to a MUD to get started.</div>';
    }
  } else {
    renderWorldTabs();
  }

  console.log(`Closed world: ${world.name}`);
}

/**
 * Load all worlds from localStorage
 */
function loadWorlds() {
  try {
    const stored = localStorage.getItem('macmush-open-worlds');
    if (stored) {
      const data = JSON.parse(stored);
      worlds = data.worlds.map(w => World.fromJSON(w));
      activeWorldId = data.activeWorldId;

      if (worlds.length > 0 && activeWorldId) {
        switchToWorld(activeWorldId);
      }
    }
  } catch (error) {
    console.error('Failed to load worlds:', error);
  }
}

/**
 * Save all worlds to localStorage
 */
function saveWorlds() {
  try {
    const data = {
      worlds: worlds.map(w => w.toJSON()),
      activeWorldId
    };
    localStorage.setItem('macmush-open-worlds', JSON.stringify(data));
  } catch (error) {
    console.error('Failed to save worlds:', error);
  }
}

/**
 * Render world tabs UI
 */
function renderWorldTabs() {
  const tabList = document.getElementById('world-tab-list');
  if (!tabList) return;

  if (worlds.length === 0) {
    tabList.innerHTML = '<div class="empty-tabs">No worlds open</div>';
    return;
  }

  tabList.innerHTML = worlds.map(world => `
    <div class="world-tab ${world.id === activeWorldId ? 'active' : ''}" data-world-id="${world.id}">
      <div class="world-tab-status ${world.isConnected ? 'connected' : ''}"></div>
      ${world.useTls ? '<span class="world-tab-secure" title="Secure TLS connection">🔒</span>' : ''}
      <span class="world-tab-name" title="${escapeHtml(world.name)}">${escapeHtml(world.name)}</span>
      <button class="world-tab-close" data-world-id="${world.id}" title="Close World">×</button>
    </div>
  `.trim()).join('');
}

/**
 * Handle new world button click
 */
function handleNewWorld() {
  // Show connection dialog
  connectionDialog.style.display = 'flex';
  document.getElementById('world-name').value = '';
  document.getElementById('world-host').value = '';
  document.getElementById('world-port').value = '';
  document.getElementById('world-name').focus();
}

/**
 * Render output buffer for active world
 */
function renderOutputBuffer() {
  const world = getActiveWorld();
  if (!world) {
    outputDisplay.innerHTML = '<div class="empty-output">No worlds open. Connect to a MUD to get started.</div>';
    return;
  }

  outputDisplay.innerHTML = '';
  for (const line of world.outputBuffer) {
    const lineDiv = document.createElement('div');
    lineDiv.className = `output-line ${line.type}`;
    lineDiv.innerHTML = line.html;
    outputDisplay.appendChild(lineDiv);
  }

  // Scroll to bottom if auto-scroll enabled
  if (world.isAutoScrollEnabled) {
    outputDisplay.scrollTop = outputDisplay.scrollHeight;
  }
}

/**
 * Load saved worlds from localStorage
 */
function loadSavedWorlds() {
  try {
    const saved = localStorage.getItem('macmush-worlds');
    if (saved) {
      savedWorlds = JSON.parse(saved);
      renderSavedWorldsList();
    }
  } catch (error) {
    console.error('Failed to load saved worlds from localStorage:', error);
  }
}

/**
 * Save worlds to localStorage
 */
function saveSavedWorlds() {
  try {
    localStorage.setItem('macmush-worlds', JSON.stringify(savedWorlds));
  } catch (error) {
    console.error('Failed to save worlds to localStorage:', error);
  }
}

/**
 * Load recent connections from localStorage
 */
function loadRecentConnections() {
  try {
    const saved = localStorage.getItem('macmush-recent');
    if (saved) {
      recentConnections = JSON.parse(saved);
    }
  } catch (error) {
    console.error('Failed to load recent connections from localStorage:', error);
  }
}

/**
 * Save recent connections to localStorage
 */
function saveRecentConnections() {
  try {
    localStorage.setItem('macmush-recent', JSON.stringify(recentConnections));
  } catch (error) {
    console.error('Failed to save recent connections to localStorage:', error);
  }
}

/**
 * Load aliases from localStorage
 */
async function loadAliases() {
  try {
    const backendAliases = await invoke('list_aliases');
    // Normalize alias data from backend format to frontend format
    aliases = backendAliases.map(alias => normalizeAlias(alias));
    renderAliasList();
  } catch (error) {
    console.error('Failed to load aliases:', error);
    appendOutput(`❌ Failed to load aliases: ${error}`, 'error');
  }
}

/**
 * Normalize alias from backend format to frontend format
 */
function normalizeAlias(alias) {
  // Extract command/script from action enum
  let command = '';
  let script = '';

  if (alias.action) {
    if (alias.action.SendCommand) {
      command = alias.action.SendCommand;
    } else if (alias.action.ExecuteScript) {
      script = alias.action.ExecuteScript;
    }
  }

  return {
    ...alias,
    command,
    script
  };
}

/**
 * Render saved worlds list
 */
function renderSavedWorldsList() {
  if (!savedWorldsList) return;

  if (savedWorlds.length === 0) {
    savedWorldsList.innerHTML = '<p class="empty-state">No saved worlds yet</p>';
    return;
  }

  savedWorldsList.innerHTML = savedWorlds.map((world, index) => {
    const isFavorite = world.autoConnect || false;
    const favoriteClass = isFavorite ? 'favorite' : '';
    const favoriteIcon = isFavorite ? '⭐' : '☆';
    return `
    <div class="saved-world-item ${favoriteClass}" data-index="${index}">
      <div class="saved-world-info">
        <div class="saved-world-name">${world.name}</div>
        <div class="saved-world-address">${world.host}:${world.port}</div>
      </div>
      <div class="saved-world-actions">
        <button class="btn-icon favorite-world" data-index="${index}" title="${isFavorite ? 'Remove from auto-connect' : 'Auto-connect on startup'}">${favoriteIcon}</button>
        <button class="btn-icon edit-world" data-index="${index}" title="Edit">✏️</button>
        <button class="btn-icon delete-world" data-index="${index}" title="Delete">🗑️</button>
      </div>
    </div>
  `;
  }).join('');

  // Add event listeners for quick connect
  document.querySelectorAll('.saved-world-item').forEach(item => {
    const info = item.querySelector('.saved-world-info');
    info.addEventListener('click', handleQuickConnect);
  });

  // Add event listeners for favorite/edit/delete
  document.querySelectorAll('.favorite-world').forEach(btn => {
    btn.addEventListener('click', handleFavoriteWorld);
  });
  document.querySelectorAll('.edit-world').forEach(btn => {
    btn.addEventListener('click', handleEditWorld);
  });
  document.querySelectorAll('.delete-world').forEach(btn => {
    btn.addEventListener('click', handleDeleteWorld);
  });
}

/**
 * Handle favorite world toggle
 */
function handleFavoriteWorld(event) {
  event.stopPropagation(); // Prevent triggering quick connect
  const index = parseInt(event.target.dataset.index);

  // Clear all other favorites (only one can be favorite)
  savedWorlds.forEach((world, i) => {
    if (i === index) {
      world.autoConnect = !world.autoConnect;
    } else {
      world.autoConnect = false;
    }
  });

  saveSavedWorlds();
  renderSavedWorldsList();

  const world = savedWorlds[index];
  if (world.autoConnect) {
    appendOutput(`⭐ "${world.name}" will auto-connect on startup`, 'system');
  } else {
    appendOutput(`☆ Auto-connect disabled for "${world.name}"`, 'system');
  }
}

/**
 * Handle save world button click
 */
function handleSaveWorld() {
  const name = document.getElementById('world-name').value.trim();
  const host = document.getElementById('host').value.trim();
  const port = parseInt(document.getElementById('port').value);
  const timeout = parseInt(document.getElementById('timeout').value) || 30;
  const autoReconnect = document.getElementById('auto-reconnect').checked;
  const keepAlive = document.getElementById('keep-alive').checked;

  if (!name || !host || !port) {
    connectError.textContent = 'Please fill in all fields before saving';
    return;
  }

  // Check if world already exists
  const existingIndex = savedWorlds.findIndex(w => w.name === name);

  const worldData = { name, host, port, timeout, autoReconnect, keepAlive };

  if (existingIndex !== -1) {
    // Update existing world
    if (confirm(`A world named "${name}" already exists. Update it?`)) {
      savedWorlds[existingIndex] = worldData;
      saveSavedWorlds();
      renderSavedWorldsList();
      appendOutput(`✓ World profile updated: "${name}"`, 'system');
    }
  } else {
    // Add new world
    savedWorlds.push(worldData);
    saveSavedWorlds();
    renderSavedWorldsList();
    appendOutput(`✓ World profile saved: "${name}"`, 'system');
  }
}

/**
 * Handle import world file
 */
async function handleImportWorld() {
  try {
    // Use Tauri dialog to select file
    const { open } = await import('@tauri-apps/plugin-dialog');
    const selected = await open({
      multiple: false,
      filters: [{
        name: 'MACMush World Files',
        extensions: ['mcl', 'xml']
      }]
    });

    if (!selected) return; // User cancelled

    // Import world file
    const result = await invoke('import_world_file', { filePath: selected });

    if (result.success) {
      // Parse world file data
      const worldFile = JSON.parse(result.world_file);

      // Apply world configuration if present
      if (worldFile.world) {
        const w = worldFile.world;
        if (w.name) document.getElementById('world-name').value = w.name;
        if (w.site) document.getElementById('host').value = w.site;
        if (w.port) document.getElementById('port').value = w.port;
      }

      // Get active world to apply automation
      const world = getActiveWorld();
      if (world) {
        // Import triggers
        if (worldFile.triggers && worldFile.triggers.items) {
          for (const trigger of worldFile.triggers.items) {
            world.triggers.push({
              pattern: trigger.match_text,
              response: trigger.send || '',
              enabled: trigger.enabled !== false,
              isRegex: trigger.regexp || false,
              ignoreCase: trigger.ignore_case || false,
              group: trigger.group || ''
            });
          }
        }

        // Import aliases
        if (worldFile.aliases && worldFile.aliases.items) {
          for (const alias of worldFile.aliases.items) {
            world.aliases.push({
              pattern: alias.match_text,
              replacement: alias.send || '',
              enabled: alias.enabled !== false,
              isRegex: alias.regexp || false,
              ignoreCase: alias.ignore_case !== false
            });
          }
        }

        // Import timers
        if (worldFile.timers && worldFile.timers.items) {
          for (const timer of worldFile.timers.items) {
            world.timers.push({
              name: timer.name,
              enabled: timer.enabled !== false,
              interval: timer.interval_seconds || 60,
              command: timer.send || ''
            });
          }
        }

        // Import macros
        if (worldFile.macros && worldFile.macros.items) {
          for (const macro of worldFile.macros.items) {
            world.macros[macro.key] = macro.send;
          }
        }

        // Import variables
        if (worldFile.variables && worldFile.variables.items) {
          for (const variable of worldFile.variables.items) {
            world.variables[variable.name] = variable.value;
          }
        }

        // Update UI
        renderTriggersList();
        renderAliasesList();
        renderTimersList();
        renderMacrosList();
        renderVariablesList();
        saveWorlds();
      }

      appendOutput(`✓ Imported ${result.trigger_count} triggers, ${result.alias_count} aliases, ${result.timer_count} timers`, 'system');
      connectError.textContent = '';
      connectError.style.color = 'var(--color-success)';
      connectError.textContent = `Successfully imported world file with ${result.trigger_count} triggers, ${result.alias_count} aliases, ${result.timer_count} timers`;
      setTimeout(() => {
        connectError.textContent = '';
      }, 5000);
    }
  } catch (error) {
    console.error('Import error:', error);
    connectError.textContent = `Import failed: ${error}`;
  }
}

/**
 * Handle export world file
 */
async function handleExportWorld() {
  try {
    const world = getActiveWorld();
    if (!world) {
      connectError.textContent = 'No active world to export';
      return;
    }

    // Build world file structure
    const worldFile = {
      world: {
        name: world.name,
        site: world.host,
        port: world.port,
        enable_triggers: true,
        enable_aliases: true,
        enable_timers: true
      },
      triggers: world.triggers.length > 0 ? {
        items: world.triggers.map(t => ({
          match_text: t.pattern,
          enabled: t.enabled,
          regexp: t.isRegex || false,
          ignore_case: t.ignoreCase || false,
          group: t.group || '',
          sequence: 100,
          send: t.response
        }))
      } : null,
      aliases: world.aliases.length > 0 ? {
        items: world.aliases.map(a => ({
          match_text: a.pattern,
          enabled: a.enabled,
          regexp: a.isRegex || false,
          ignore_case: a.ignoreCase !== false,
          sequence: 100,
          send: a.replacement
        }))
      } : null,
      timers: world.timers.length > 0 ? {
        items: world.timers.map(t => ({
          name: t.name,
          enabled: t.enabled,
          interval_seconds: t.interval,
          send: t.command
        }))
      } : null,
      macros: Object.keys(world.macros).length > 0 ? {
        items: Object.entries(world.macros).map(([key, send]) => ({
          key,
          send
        }))
      } : null,
      variables: Object.keys(world.variables).length > 0 ? {
        items: Object.entries(world.variables).map(([name, value]) => ({
          name,
          value: String(value)
        }))
      } : null
    };

    // Use Tauri dialog to save file
    const { save } = await import('@tauri-apps/plugin-dialog');
    const filePath = await save({
      defaultPath: `${world.name}.mcl`,
      filters: [{
        name: 'MACMush World Files',
        extensions: ['mcl', 'xml']
      }]
    });

    if (!filePath) return; // User cancelled

    // Export world file
    const result = await invoke('export_world_file', {
      filePath,
      worldData: JSON.stringify(worldFile)
    });

    if (result.success) {
      appendOutput(`✓ World exported to: ${result.file_path}`, 'system');
      connectError.style.color = 'var(--color-success)';
      connectError.textContent = `Successfully exported world to: ${result.file_path}`;
      setTimeout(() => {
        connectError.textContent = '';
      }, 5000);
    }
  } catch (error) {
    console.error('Export error:', error);
    connectError.textContent = `Export failed: ${error}`;
  }
}

/**
 * Handle quick connect from saved world
 */
function handleQuickConnect(event) {
  const item = event.target.closest('.saved-world-item');
  const index = parseInt(item.dataset.index);
  const world = savedWorlds[index];

  // Populate form fields
  document.getElementById('world-name').value = world.name;
  document.getElementById('host').value = world.host;
  document.getElementById('port').value = world.port;
  document.getElementById('timeout').value = world.timeout || 30;
  document.getElementById('auto-reconnect').checked = world.autoReconnect || false;
  document.getElementById('keep-alive').checked = world.keepAlive !== false; // Default true

  // Auto-connect
  connectForm.dispatchEvent(new Event('submit'));
}

/**
 * Handle edit world button click
 */
function handleEditWorld(event) {
  event.stopPropagation(); // Prevent triggering quick connect
  const index = parseInt(event.target.dataset.index);
  const world = savedWorlds[index];

  // Populate form fields
  document.getElementById('world-name').value = world.name;
  document.getElementById('host').value = world.host;
  document.getElementById('port').value = world.port;
  document.getElementById('timeout').value = world.timeout || 30;
  document.getElementById('auto-reconnect').checked = world.autoReconnect || false;
  document.getElementById('keep-alive').checked = world.keepAlive !== false; // Default true

  // Focus the name field
  document.getElementById('world-name').focus();
}

/**
 * Handle delete world button click
 */
function handleDeleteWorld(event) {
  event.stopPropagation(); // Prevent triggering quick connect
  const index = parseInt(event.target.dataset.index);
  const world = savedWorlds[index];

  if (!confirm(`Delete saved world "${world.name}"?`)) {
    return;
  }

  savedWorlds.splice(index, 1);
  saveSavedWorlds();
  renderSavedWorldsList();
  appendOutput(`✗ World profile deleted: "${world.name}"`, 'system');
}

/**
 * Render recent connections list
 */
function renderRecentConnectionsList() {
  if (!recentConnectionsList) return;

  if (recentConnections.length === 0) {
    recentConnectionsList.innerHTML = '<p class="empty-state">No recent connections</p>';
    return;
  }

  recentConnectionsList.innerHTML = recentConnections.map((connection, index) => {
    const timeAgo = getTimeAgo(connection.timestamp);
    return `
    <div class="recent-connection-item" data-index="${index}">
      <div class="recent-connection-header">
        <div class="recent-connection-name">${connection.name}</div>
        <div class="recent-connection-time">${timeAgo}</div>
      </div>
      <div class="recent-connection-address">${connection.host}:${connection.port}</div>
    </div>
  `;
  }).join('');

  // Add event listeners for quick connect
  document.querySelectorAll('.recent-connection-item').forEach(item => {
    item.addEventListener('click', handleRecentConnect);
  });
}

/**
 * Get human-readable time ago string
 */
function getTimeAgo(timestamp) {
  const now = Date.now();
  const diff = now - timestamp;
  const seconds = Math.floor(diff / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);

  if (days > 0) return `${days}d ago`;
  if (hours > 0) return `${hours}h ago`;
  if (minutes > 0) return `${minutes}m ago`;
  return 'just now';
}

/**
 * Add connection to recent history
 */
function addRecentConnection(name, host, port) {
  // Remove duplicate if exists
  recentConnections = recentConnections.filter(
    c => !(c.name === name && c.host === host && c.port === port)
  );

  // Add to beginning
  recentConnections.unshift({
    name,
    host,
    port,
    timestamp: Date.now()
  });

  // Keep only last 10
  if (recentConnections.length > 10) {
    recentConnections = recentConnections.slice(0, 10);
  }

  saveRecentConnections();
  renderRecentConnectionsList();
}

/**
 * Handle quick connect from recent connection
 */
function handleRecentConnect(event) {
  const item = event.target.closest('.recent-connection-item');
  const index = parseInt(item.dataset.index);
  const connection = recentConnections[index];

  // Populate form fields
  document.getElementById('world-name').value = connection.name;
  document.getElementById('host').value = connection.host;
  document.getElementById('port').value = connection.port;

  // Auto-connect
  connectForm.dispatchEvent(new Event('submit'));
}

/**
 * Handle clear recent connections
 */
function handleClearRecent() {
  if (!confirm('Clear all recent connections?')) {
    return;
  }

  recentConnections = [];
  saveRecentConnections();
  renderRecentConnectionsList();
  appendOutput('✓ Recent connections cleared', 'system');
}

/**
 * Load triggers from localStorage
 */
async function loadTriggers() {
  try {
    const backendTriggers = await invoke('list_triggers');
    // Normalize trigger data from backend format to frontend format
    triggers = backendTriggers.map(trigger => normalizeTrigger(trigger));
    renderTriggerList();
  } catch (error) {
    console.error('Failed to load triggers:', error);
    appendOutput(`❌ Failed to load triggers: ${error}`, 'error');
  }
}

/**
 * Normalize trigger from backend format to frontend format
 */
function normalizeTrigger(trigger) {
  // Extract command/script from action enum
  let command = '';
  let script = '';

  if (trigger.action) {
    if (trigger.action.SendCommand) {
      command = trigger.action.SendCommand;
    } else if (trigger.action.ExecuteScript) {
      script = trigger.action.ExecuteScript;
    }
  }

  return {
    ...trigger,
    command,
    script,
    match_count: trigger.match_count || 0
  };
}

/**
 * (Removed - now saving via backend CRUD commands)
 */
function saveTriggers() {
  try {
    // Deprecated - triggers now saved via backend CRUD commands
  } catch (error) {
    console.error('Failed to save triggers to localStorage:', error);
  }
}

/**
 * Load command history from localStorage
 */
function loadCommandHistory() {
  try {
    const saved = localStorage.getItem('macmush-command-history');
    if (saved) {
      commandHistory = JSON.parse(saved);
    }
  } catch (error) {
    console.error('Failed to load command history from localStorage:', error);
  }
}

/**
 * Save command history to localStorage
 */
function saveCommandHistory() {
  try {
    localStorage.setItem('macmush-command-history', JSON.stringify(commandHistory));
  } catch (error) {
    console.error('Failed to save command history to localStorage:', error);
  }
}

/**
 * Add command to history
 */
function addToCommandHistory(command) {
  // Don't add empty commands or duplicates of the last command
  if (!command.trim() || command === commandHistory[commandHistory.length - 1]) {
    return;
  }

  // Add to history
  commandHistory.push(command);

  // Enforce size limit
  if (commandHistory.length > maxCommandHistory) {
    commandHistory.shift(); // Remove oldest
  }

  // Save to localStorage
  saveCommandHistory();

  // Reset navigation state
  commandHistoryIndex = -1;
  currentCommand = '';
}

/**
 * Navigate up in command history (older commands)
 */
function navigateHistoryUp() {
  if (commandHistory.length === 0) {
    return;
  }

  // Store current input if starting navigation
  if (commandHistoryIndex === -1) {
    currentCommand = commandInput.value;
    commandHistoryIndex = commandHistory.length;
  }

  // Move up in history
  if (commandHistoryIndex > 0) {
    commandHistoryIndex--;
    commandInput.value = commandHistory[commandHistoryIndex];
  }
}

/**
 * Navigate down in command history (newer commands)
 */
function navigateHistoryDown() {
  if (commandHistoryIndex === -1) {
    return; // Not currently navigating
  }

  commandHistoryIndex++;

  if (commandHistoryIndex >= commandHistory.length) {
    // Reached the end, restore current command
    commandInput.value = currentCommand;
    commandHistoryIndex = -1;
    currentCommand = '';
  } else {
    commandInput.value = commandHistory[commandHistoryIndex];
  }
}

/**
 * Start history search mode (Ctrl+R)
 */
function startHistorySearch() {
  if (commandHistory.length === 0) {
    return;
  }

  isSearchingHistory = true;
  historySearchQuery = '';
  historySearchMatches = [];
  historySearchIndex = -1;

  // Update UI to show search mode
  commandInput.placeholder = '(reverse-i-search): ';
  commandInput.value = '';
  commandInput.classList.add('history-search-mode');
}

/**
 * Update history search
 */
function updateHistorySearch(query) {
  historySearchQuery = query.toLowerCase();

  // Find all matching commands (search backwards from most recent)
  historySearchMatches = [];
  for (let i = commandHistory.length - 1; i >= 0; i--) {
    if (commandHistory[i].toLowerCase().includes(historySearchQuery)) {
      historySearchMatches.push(i);
    }
  }

  // Show first match
  if (historySearchMatches.length > 0) {
    historySearchIndex = 0;
    const matchIndex = historySearchMatches[0];
    commandInput.placeholder = `(reverse-i-search)\`${historySearchQuery}': ${commandHistory[matchIndex]}`;
  } else {
    commandInput.placeholder = `(failed reverse-i-search)\`${historySearchQuery}': `;
  }
}

/**
 * Navigate to next history search match
 */
function nextHistorySearchMatch() {
  if (!isSearchingHistory || historySearchMatches.length === 0) {
    return;
  }

  historySearchIndex = (historySearchIndex + 1) % historySearchMatches.length;
  const matchIndex = historySearchMatches[historySearchIndex];
  commandInput.placeholder = `(reverse-i-search)\`${historySearchQuery}': ${commandHistory[matchIndex]}`;
}

/**
 * Exit history search mode
 */
function exitHistorySearch(accept = true) {
  if (!isSearchingHistory) {
    return;
  }

  isSearchingHistory = false;
  commandInput.classList.remove('history-search-mode');
  commandInput.placeholder = 'Enter command...';

  if (accept && historySearchMatches.length > 0) {
    // Accept current match
    const matchIndex = historySearchMatches[historySearchIndex];
    commandInput.value = commandHistory[matchIndex];
  } else {
    // Cancel search
    commandInput.value = '';
  }

  // Reset search state
  historySearchQuery = '';
  historySearchMatches = [];
  historySearchIndex = -1;
}

/**
 * Update UI based on connection status
 */
function updateConnectionStatus(connected, worldName = null) {
  isConnected = connected;

  if (connected) {
    // Show client interface, hide connection dialog
    connectionDialog.style.display = 'none';
    clientInterface.style.display = 'flex';

    // Update status
    statusIndicator.className = 'status-dot connected';
    statusText.textContent = worldName ? `Connected to ${worldName}` : 'Connected';

    // Focus command input
    commandInput.focus();
  } else {
    // Show connection dialog, hide client interface
    connectionDialog.style.display = 'flex';
    clientInterface.style.display = 'none';

    // Update status
    statusIndicator.className = 'status-dot disconnected';
    statusText.textContent = 'Disconnected';
  }
}

/**
 * Check if text matches any enabled triggers and update statistics
 */
function checkTriggerMatches(text) {
  let matchedAny = false;

  triggers.forEach((trigger, index) => {
    if (!trigger.enabled) return;

    try {
      const regex = new RegExp(trigger.pattern);
      if (regex.test(text)) {
        triggers[index].match_count = (triggers[index].match_count || 0) + 1;
        matchedAny = true;

        // Log trigger match (optional debug output)
        console.log(`⚡ Trigger fired: "${trigger.name}" (Match #${triggers[index].match_count})`);
      }
    } catch (error) {
      // Invalid regex, skip silently
    }
  });

  if (matchedAny) {
    saveTriggers();
    renderTriggerList();
  }

  return matchedAny;
}

/**
 * Append text to output display with ANSI color support
 */
function appendOutput(text, className = '') {
  // Check output filters
  const messageType = className || 'mud';
  if (!outputFilters[messageType]) {
    return; // Skip this message type
  }

  const line = document.createElement('div');
  line.className = `output-line ${className}`;
  line.setAttribute('data-type', messageType);

  // Check if text already has backend highlights applied (contains HTML)
  const hasBackendHighlights = text.includes('<span style=');

  // Parse ANSI codes if no special className (system/command/error have their own colors)
  if (!className || className === '') {
    // If backend highlights are present, skip ANSI parsing and use innerHTML directly
    if (hasBackendHighlights) {
      line.innerHTML = text;
    } else {
      const segments = parseAnsi(text);

      segments.forEach(segment => {
        if (segment.text) {
          const span = document.createElement('span');
          span.className = 'ansi-text';

          // Apply highlighting and variable capture to MUD text
          const highlightedText = processHighlights(segment.text);

          if (highlightedText !== segment.text) {
            // Highlighting was applied, use innerHTML
            span.innerHTML = highlightedText;
          } else {
            // No highlighting, use textContent
            span.textContent = segment.text;
          }

          // Apply ANSI styles
          const css = styleToCSS(segment.style);
          Object.assign(span.style, css);

          line.appendChild(span);
        }
      });
    }
  } else {
    // For system/command/error messages, check for backend highlights
    if (hasBackendHighlights) {
      line.innerHTML = text;
    } else {
      const highlightedText = processHighlights(text);
      if (highlightedText !== text) {
        line.innerHTML = highlightedText;
      } else {
        line.textContent = text;
      }
    }
  }

  outputDisplay.appendChild(line);

  // Log output if logging is active
  if (isLogging) {
    logOutputEntry(text, messageType);
  }

  // Enforce scrollback buffer limit
  const lines = outputDisplay.children;
  if (lines.length > maxScrollbackLines) {
    const excessLines = lines.length - maxScrollbackLines;
    for (let i = 0; i < excessLines; i++) {
      outputDisplay.removeChild(lines[0]);
    }
  }

  // Auto-scroll to bottom if enabled
  if (isAutoScrollEnabled) {
    outputDisplay.scrollTop = outputDisplay.scrollHeight;
  }
}

/**
 * Clear output display
 */
function clearOutput() {
  outputDisplay.innerHTML = '';
  clearSearch();
}

/**
 * Search output text
 */
function searchOutput(searchText) {
  clearSearch();

  if (!searchText.trim()) {
    return;
  }

  const lines = outputDisplay.querySelectorAll('.output-line');
  searchMatches = [];

  lines.forEach(line => {
    const text = line.textContent.toLowerCase();
    const search = searchText.toLowerCase();

    if (text.includes(search)) {
      // Highlight all occurrences in this line
      highlightTextInElement(line, searchText);
      const highlights = line.querySelectorAll('.search-highlight');
      highlights.forEach(h => searchMatches.push(h));
    }
  });

  if (searchMatches.length > 0) {
    currentSearchIndex = 0;
    searchMatches[0].classList.add('current');
    searchMatches[0].scrollIntoView({ behavior: 'smooth', block: 'center' });
  }
}

/**
 * Highlight text in element
 */
function highlightTextInElement(element, searchText) {
  const walker = document.createTreeWalker(
    element,
    NodeFilter.SHOW_TEXT,
    null,
    false
  );

  const textNodes = [];
  while (walker.nextNode()) {
    textNodes.push(walker.currentNode);
  }

  const regex = new RegExp(`(${escapeRegExp(searchText)})`, 'gi');

  textNodes.forEach(node => {
    if (node.nodeValue.match(regex)) {
      const span = document.createElement('span');
      span.innerHTML = node.nodeValue.replace(regex, '<span class="search-highlight">$1</span>');
      node.parentNode.replaceChild(span, node);
    }
  });
}

/**
 * Escape regex special characters
 */
function escapeRegExp(string) {
  return string.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/**
 * Clear search highlights
 */
function clearSearch() {
  const highlights = outputDisplay.querySelectorAll('.search-highlight');
  highlights.forEach(highlight => {
    const text = highlight.textContent;
    const textNode = document.createTextNode(text);
    highlight.parentNode.replaceChild(textNode, highlight);
  });

  // Normalize text nodes
  const lines = outputDisplay.querySelectorAll('.output-line');
  lines.forEach(line => line.normalize());

  searchMatches = [];
  currentSearchIndex = -1;
}

/**
 * Navigate to next search match
 */
function searchNext() {
  if (searchMatches.length === 0) return;

  searchMatches[currentSearchIndex].classList.remove('current');
  currentSearchIndex = (currentSearchIndex + 1) % searchMatches.length;
  searchMatches[currentSearchIndex].classList.add('current');
  searchMatches[currentSearchIndex].scrollIntoView({ behavior: 'smooth', block: 'center' });
}

/**
 * Navigate to previous search match
 */
function searchPrevious() {
  if (searchMatches.length === 0) return;

  searchMatches[currentSearchIndex].classList.remove('current');
  currentSearchIndex = (currentSearchIndex - 1 + searchMatches.length) % searchMatches.length;
  searchMatches[currentSearchIndex].classList.add('current');
  searchMatches[currentSearchIndex].scrollIntoView({ behavior: 'smooth', block: 'center' });
}

/**
 * Handle filter toggle
 */
function handleFilterToggle(filterType, enabled) {
  outputFilters[filterType] = enabled;

  // Re-render existing lines with filter
  const lines = Array.from(outputDisplay.querySelectorAll('.output-line'));
  lines.forEach(line => {
    const lineType = line.getAttribute('data-type');
    if (lineType === filterType) {
      line.style.display = enabled ? '' : 'none';
    }
  });
}

/**
 * Handle scroll event to show/hide scroll-to-bottom button
 */
function handleOutputScroll() {
  const isScrolledToBottom =
    outputDisplay.scrollHeight - outputDisplay.scrollTop <= outputDisplay.clientHeight + 50;

  if (isScrolledToBottom) {
    isAutoScrollEnabled = true;
    scrollToBottomBtn.style.display = 'none';
  } else {
    isAutoScrollEnabled = false;
    scrollToBottomBtn.style.display = 'flex';
  }
}

/**
 * Scroll to bottom of output
 */
function scrollToBottom() {
  outputDisplay.scrollTop = outputDisplay.scrollHeight;
  isAutoScrollEnabled = true;
  scrollToBottomBtn.style.display = 'none';
}

/**
 * Toggle logging on/off
 */
async function toggleLogging() {
  if (isLogging) {
    // Stop logging
    await stopLogging();
  } else {
    // Start logging
    await startLogging();
  }
}

/**
 * Start logging session output
 */
async function startLogging() {
  try {
    // Check if connected
    if (!isConnected) {
      appendOutput('⚠️ Connect to a world first to start logging', 'system');
      return;
    }

    // Get world name from connection
    const worldName = document.getElementById('world-name').value || 'unknown';

    // Start logging with current format
    const result = await invoke('start_logging', {
      request: {
        world_name: worldName,
        format: logFormat
      }
    });

    isLogging = true;
    currentLogFile = result.log_file;

    // Update UI
    const toggleBtn = document.getElementById('toggle-logging-btn');
    const openBtn = document.getElementById('open-logs-btn');

    toggleBtn.textContent = '⏸️';
    toggleBtn.title = 'Stop Logging';
    toggleBtn.classList.add('logging-active');
    openBtn.style.display = 'inline-block';

    appendOutput(`✓ Logging started: ${currentLogFile}`, 'system');
  } catch (error) {
    appendOutput(`❌ Failed to start logging: ${error}`, 'error');
    console.error('Start logging error:', error);
  }
}

/**
 * Stop logging session
 */
async function stopLogging() {
  try {
    await invoke('stop_logging');

    isLogging = false;
    const previousLogFile = currentLogFile;
    currentLogFile = null;

    // Update UI
    const toggleBtn = document.getElementById('toggle-logging-btn');
    toggleBtn.textContent = '📝';
    toggleBtn.title = 'Start Logging';
    toggleBtn.classList.remove('logging-active');

    appendOutput(`✓ Logging stopped: ${previousLogFile}`, 'system');
  } catch (error) {
    appendOutput(`❌ Failed to stop logging: ${error}`, 'error');
    console.error('Stop logging error:', error);
  }
}

/**
 * Open logs folder in file manager
 */
async function openLogsFolder() {
  try {
    await invoke('open_logs_folder');
  } catch (error) {
    appendOutput(`❌ Failed to open logs folder: ${error}`, 'error');
    console.error('Open logs folder error:', error);
  }
}

/**
 * Log output entry (called automatically when output is added)
 */
async function logOutputEntry(text, messageType) {
  if (!isLogging) return;

  // Check log filters
  if (!logFilters[messageType]) return;

  try {
    await invoke('write_log_entry', {
      text,
      messageType
    });
  } catch (error) {
    console.error('Failed to write log entry:', error);
  }
}

// ============================================================================
// MACROS & KEYBOARD SHORTCUTS
// ============================================================================

/**
 * Load macros from localStorage
 */
function loadMacros() {
  try {
    const stored = localStorage.getItem('macros');
    if (stored) {
      macros = JSON.parse(stored);
    }
  } catch (error) {
    console.error('Failed to load macros:', error);
  }
}

/**
 * Save macros to localStorage
 */
function saveMacros() {
  try {
    localStorage.setItem('macros', JSON.stringify(macros));
  } catch (error) {
    console.error('Failed to save macros:', error);
  }
}

/**
 * Handle keyboard shortcuts and macros
 */
function handleKeyboardShortcut(event) {
  // Ignore if typing in an input field (except command input for F-keys)
  const target = event.target;
  const isInputField = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA';

  // Allow F-keys even in input fields, but prevent other shortcuts
  const isFunctionKey = event.key.startsWith('F') && /^F([1-9]|1[0-2])$/.test(event.key);

  if (isInputField && !isFunctionKey) {
    return;
  }

  // Build key combination string
  const modifiers = [];
  if (event.ctrlKey) modifiers.push('Ctrl');
  if (event.altKey) modifiers.push('Alt');
  if (event.shiftKey && !isFunctionKey) modifiers.push('Shift');

  // Get the key name
  let keyName = event.key;

  // Normalize key names
  if (keyName.length === 1) {
    keyName = keyName.toUpperCase();
  }

  // Build combination string
  const combination = modifiers.length > 0
    ? `${modifiers.join('+')}+${keyName}`
    : keyName;

  // Handle numpad navigation keys (when connected to a world)
  if (event.code && event.code.startsWith('Numpad')) {
    const world = getActiveWorld();
    if (world && world.isConnected) {
      event.preventDefault();
      handleKeypadPress(event);
      return;
    }
  }

  // Check if this combination has a macro
  if (macros[combination]) {
    event.preventDefault();
    executeMacro(macros[combination]);
  }
}

/**
 * Handle numpad key press for navigation
 */
async function handleKeypadPress(event) {
  // Map JavaScript event.code to keypad key names
  const keypadKeyMap = {
    'Numpad0': '0',
    'Numpad1': '1',
    'Numpad2': '2',
    'Numpad3': '3',
    'Numpad4': '4',
    'Numpad5': '5',
    'Numpad6': '6',
    'Numpad7': '7',
    'Numpad8': '8',
    'Numpad9': '9',
    'NumpadDecimal': 'dot',
    'NumpadDivide': 'slash',
    'NumpadMultiply': 'star',
    'NumpadSubtract': 'minus',
    'NumpadAdd': 'plus',
    'NumpadEnter': 'enter',
  };

  const keypadKey = keypadKeyMap[event.code];
  if (!keypadKey) {
    return; // Not a recognized keypad key
  }

  try {
    await invoke('execute_keypad_key', {
      request: {
        key: keypadKey,
        ctrl: event.ctrlKey,
      },
    });
  } catch (error) {
    console.error('Failed to execute keypad key:', error);
    appendOutput(error, 'error');
  }
}

/**
 * Parse speed walking command (e.g., "3n" -> "n;n;n")
 */
function parseSpeedWalk(command) {
  // Match pattern: number followed by direction
  // Supports: n, s, e, w, ne, nw, se, sw, u, d
  const speedWalkPattern = /^(\d+)([nsewud]|ne|nw|se|sw)$/i;
  const match = command.match(speedWalkPattern);

  if (!match) {
    return command; // Not a speed walk command
  }

  const count = parseInt(match[1]);
  const direction = match[2].toLowerCase();

  // Build repeated command
  const commands = [];
  for (let i = 0; i < count; i++) {
    commands.push(direction);
  }

  return commands.join(';');
}

/**
 * Execute macro command (handles multi-command with semicolons)
 */
async function executeMacro(command) {
  if (!isConnected) {
    appendOutput('⚠️ Not connected to a world', 'system');
    return;
  }

  // Parse speed walking if enabled
  if (speedWalkKeys) {
    command = parseSpeedWalk(command);
  }

  // Split by semicolon for multi-command macros
  const commands = command.split(';').map(cmd => cmd.trim()).filter(cmd => cmd.length > 0);

  // Execute each command with slight delay
  for (let i = 0; i < commands.length; i++) {
    const cmd = commands[i];

    // Add to command history
    commandHistory.unshift(cmd);
    if (commandHistory.length > MAX_HISTORY) {
      commandHistory.pop();
    }
    historyIndex = -1;
    saveCommandHistory();

    // Show in output
    appendOutput(`> ${cmd}`, 'command');

    // Send to MUD
    try {
      await invoke('send_command', { command: cmd });
    } catch (error) {
      appendOutput(`❌ Failed to send command: ${error}`, 'error');
      console.error('Send command error:', error);
    }

    // Small delay between commands (except for last one)
    if (i < commands.length - 1) {
      await new Promise(resolve => setTimeout(resolve, 100));
    }
  }
}

/**
 * Add or update a macro
 */
function addMacro(keyCombo, command) {
  macros[keyCombo] = command;
  saveMacros();
}

/**
 * Remove a macro
 */
function removeMacro(keyCombo) {
  delete macros[keyCombo];
  saveMacros();
}

// ============================================================================
// TEXT HIGHLIGHTING & VARIABLES
// ============================================================================

/**
 * Load highlights from localStorage
 */
async function loadHighlights() {
  try {
    highlights = await invoke('list_highlights');
    renderHighlightList();
  } catch (error) {
    console.error('Failed to load highlights:', error);
    appendOutput(`❌ Failed to load highlights: ${error}`, 'error');
  }
}

/**
 * Apply highlight matches from backend
 * Takes text and array of [start, end, style] tuples from HighlightMatched event
 * Returns HTML with inline styles applied
 */
function applyHighlightMatches(text, matches) {
  if (!matches || matches.length === 0) {
    return text;
  }

  // Sort matches by start position (descending) to apply from end to start
  // This prevents position shifts as we insert HTML tags
  const sorted = [...matches].sort((a, b) => b[0] - a[0]);

  let result = text;
  for (const match of sorted) {
    const start = match[0];
    const end = match[1];
    const style = match[2];

    // Extract the parts
    const before = result.substring(0, start);
    const highlighted = result.substring(start, end);
    const after = result.substring(end);

    // Build style string
    const styleStr = `color: ${style.color};${style.bold ? ' font-weight: bold;' : ''}${style.italic ? ' font-style: italic;' : ''}${style.underline ? ' text-decoration: underline;' : ''}`;

    // Reconstruct with span
    result = before + `<span style="${styleStr}">${highlighted}</span>` + after;
  }

  return result;
}

/**
 * Process text for highlighting and variable capture
 * Returns HTML with inline styles for highlights
 */
function processHighlights(text) {
  let result = text;
  let captured = false;

  // Check each enabled highlight
  for (const highlight of highlights) {
    if (highlight.enabled === false) continue;

    try {
      const regex = new RegExp(highlight.pattern, 'g');
      const matches = [...text.matchAll(regex)];

      if (matches.length > 0) {
        // Capture variables if defined
        if (highlight.variables && highlight.variables.length > 0) {
          for (const match of matches) {
            highlight.variables.forEach((varName, index) => {
              if (varName && match[index + 1] !== undefined) {
                variables[varName] = match[index + 1];
                captured = true;
              }
            });
          }
        }

        // Apply highlighting
        result = result.replace(regex, (match) => {
          const style = `color: ${highlight.color || '#ffff00'}; ${highlight.bold ? 'font-weight: bold;' : ''} ${highlight.italic ? 'font-style: italic;' : ''} ${highlight.underline ? 'text-decoration: underline;' : ''}`;
          return `<span style="${style}">${match}</span>`;
        });
      }
    } catch (error) {
      console.error(`Error in highlight "${highlight.name}":`, error);
    }
  }

  // Update variables panel if any variables were captured
  if (captured) {
    renderVariablesList();
  }

  return result;
}

/**
 * Add or update a highlight
 */
function addHighlight(highlight) {
  if (editingHighlightIndex !== null) {
    highlights[editingHighlightIndex] = highlight;
  } else {
    highlights.push(highlight);
  }
  saveHighlights();
}

/**
 * Remove a highlight
 */
function removeHighlight(index) {
  highlights.splice(index, 1);
  saveHighlights();
}

/**
 * Toggle highlight enabled state
 */
function toggleHighlight(index) {
  highlights[index].enabled = !highlights[index].enabled;
  saveHighlights();
}

// ============================================================================
// STATUS BAR
// ============================================================================

/**
 * Start status bar updates
 */
function startStatusBar() {
  connectionStartTime = Date.now();
  bytesSent = 0;
  bytesReceived = 0;

  // Show status bar
  statusBar.style.display = 'flex';

  // Update every second
  statusBarUpdateInterval = setInterval(updateStatusBar, 1000);

  // Initial update
  updateStatusBar();
}

/**
 * Stop status bar updates
 */
function stopStatusBar() {
  if (statusBarUpdateInterval) {
    clearInterval(statusBarUpdateInterval);
    statusBarUpdateInterval = null;
  }

  // Hide status bar
  statusBar.style.display = 'none';

  // Reset values
  connectionStartTime = null;
  bytesSent = 0;
  bytesReceived = 0;
}

/**
 * Update status bar with current info
 */
function updateStatusBar() {
  // Update uptime
  if (connectionStartTime) {
    const uptime = Date.now() - connectionStartTime;
    const hours = Math.floor(uptime / 3600000);
    const minutes = Math.floor((uptime % 3600000) / 60000);
    const seconds = Math.floor((uptime % 60000) / 1000);

    statusUptime.textContent = `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
  }

  // Update traffic (format bytes)
  statusBytesSent.textContent = formatBytes(bytesSent);
  statusBytesReceived.textContent = formatBytes(bytesReceived);

  // Update variables display
  updateStatusVariables();
}

/**
 * Update status bar variables section
 */
function updateStatusVariables() {
  // Get displayed variables that have values
  const displayedVars = displayedVariables
    .filter(varName => variables[varName] !== undefined)
    .map(varName => {
      return `
        <div class="status-var">
          <span class="status-var-name">${varName}:</span>
          <span class="status-var-value">${variables[varName]}</span>
        </div>
      `;
    })
    .join('');

  if (displayedVars) {
    statusVariables.innerHTML = displayedVars;
  } else {
    statusVariables.innerHTML = '<span class="status-label">No stats</span>';
  }
}

/**
 * Format bytes to human-readable format
 */
function formatBytes(bytes) {
  if (bytes === 0) return '0 B';

  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));

  return Math.round((bytes / Math.pow(k, i)) * 100) / 100 + ' ' + sizes[i];
}

/**
 * Track sent command for statistics
 */
function trackCommandSent(command) {
  // Estimate bytes (command + newline)
  bytesSent += command.length + 1;
}

/**
 * Track received data for statistics
 */
function trackDataReceived(data) {
  bytesReceived += data.length;
}

// ===========================
// Scripting & Automation Functions
// ===========================

/**
 * Load scripts from localStorage
 */
function loadScripts() {
  try {
    const stored = localStorage.getItem('scripts');
    if (stored) {
      scripts = JSON.parse(stored);
      renderScriptsList();
    }
  } catch (error) {
    console.error('Failed to load scripts:', error);
  }
}

/**
 * Save scripts to localStorage
 */
function saveScripts() {
  try {
    localStorage.setItem('scripts', JSON.stringify(scripts));
  } catch (error) {
    console.error('Failed to save scripts:', error);
  }
}

/**
 * Create MUD API context for script execution
 */
function createScriptAPI() {
  return {
    // Send command to MUD
    send: async (command) => {
      if (!isConnected) {
        console.warn('Cannot send command: not connected');
        return;
      }
      await sendCommand(command);
    },

    // Output to display
    output: (text, type = '') => {
      appendOutput(text, type);
    },

    // Access variables
    getVariable: (name) => variables[name],
    setVariable: (name, value) => {
      variables[name] = value;
      renderVariablesList();
      updateStatusBar();
    },
    getAllVariables: () => ({ ...variables }),

    // Trigger management
    addTrigger: (name, pattern, command, enabled = true) => {
      triggers.push({ name, pattern, command, enabled, match_count: 0 });
      saveTriggers();
      renderTriggersList();
    },
    removeTrigger: (name) => {
      const index = triggers.findIndex(t => t.name === name);
      if (index !== -1) {
        triggers.splice(index, 1);
        saveTriggers();
        renderTriggersList();
      }
    },

    // Alias management
    addAlias: (name, pattern, replacement, enabled = true) => {
      aliases.push({ name, pattern, replacement, enabled });
      saveAliases();
      renderAliasesList();
    },
    removeAlias: (name) => {
      const index = aliases.findIndex(a => a.name === name);
      if (index !== -1) {
        aliases.splice(index, 1);
        saveAliases();
        renderAliasesList();
      }
    },

    // Utility functions
    wait: (ms) => new Promise(resolve => setTimeout(resolve, ms)),
    log: (...args) => console.log('[Script]', ...args),

    // Connection info
    isConnected: () => isConnected,
    getUptime: () => connectionStartTime ? Date.now() - connectionStartTime : 0,
    getBytesSent: () => bytesSent,
    getBytesReceived: () => bytesReceived
  };
}

/**
 * Execute a script
 */
async function executeScript(scriptCode, scriptName = 'anonymous') {
  try {
    const api = createScriptAPI();

    // Create async function with API in scope
    const AsyncFunction = Object.getPrototypeOf(async function(){}).constructor;
    const scriptFn = new AsyncFunction('mud', scriptCode);

    // Execute script
    await scriptFn(api);

    console.log(`✓ Script "${scriptName}" executed successfully`);
  } catch (error) {
    appendOutput(`❌ Script error in "${scriptName}": ${error.message}`, 'error');
    console.error(`Script execution error in "${scriptName}":`, error);
  }
}

/**
 * Render scripts list
 */
function renderScriptsList() {
  const scriptList = document.getElementById('script-list');

  if (scripts.length === 0) {
    scriptList.innerHTML = '<p class="empty-state">No scripts yet</p>';
    return;
  }

  scriptList.innerHTML = scripts.map((script, index) => `
    <div class="script-item ${script.enabled ? '' : 'disabled'}">
      <div class="script-header">
        <label class="script-checkbox">
          <input type="checkbox" ${script.enabled ? 'checked' : ''}
                 onchange="toggleScriptEnabled(${index})" />
          <span class="script-name">${escapeHtml(script.name)}</span>
        </label>
        <div class="script-actions">
          <button class="btn btn-icon btn-sm" onclick="executeScriptByIndex(${index})" title="Run Script">▶️</button>
          <button class="btn btn-icon btn-sm" onclick="editScript(${index})" title="Edit Script">✏️</button>
          <button class="btn btn-icon btn-sm btn-danger" onclick="deleteScript(${index})" title="Delete Script">🗑️</button>
        </div>
      </div>
      <div class="script-info">
        <span class="script-trigger">${script.trigger || 'Manual execution'}</span>
      </div>
    </div>
  `).join('');
}

/**
 * Show script modal for creating/editing
 */
function showScriptModal(index = null) {
  editingScriptIndex = index;
  const modal = document.getElementById('script-modal');
  const form = document.getElementById('script-form');
  const title = document.querySelector('#script-modal .modal-header h2');
  const submitBtn = document.querySelector('#script-form button[type="submit"]');

  if (index !== null) {
    // Edit mode
    const script = scripts[index];
    title.textContent = 'Edit Script';
    submitBtn.textContent = 'Update Script';
    document.getElementById('script-name').value = script.name;
    document.getElementById('script-trigger').value = script.trigger || '';
    document.getElementById('script-code').value = script.code;
  } else {
    // Create mode
    title.textContent = 'Create Script';
    submitBtn.textContent = 'Create Script';
    form.reset();
  }

  document.getElementById('script-error').textContent = '';
  modal.style.display = 'flex';
}

/**
 * Hide script modal
 */
function hideScriptModal() {
  document.getElementById('script-modal').style.display = 'none';
  editingScriptIndex = null;
}

/**
 * Handle create/edit script form submission
 */
async function handleCreateScript(event) {
  event.preventDefault();

  const name = document.getElementById('script-name').value.trim();
  const trigger = document.getElementById('script-trigger').value.trim();
  const code = document.getElementById('script-code').value.trim();
  const errorEl = document.getElementById('script-error');

  if (!name) {
    errorEl.textContent = 'Script name is required';
    return;
  }

  if (!code) {
    errorEl.textContent = 'Script code is required';
    return;
  }

  // Check for duplicate names (excluding current edit)
  const duplicateIndex = scripts.findIndex((s, idx) =>
    s.name.toLowerCase() === name.toLowerCase() && idx !== editingScriptIndex
  );

  if (duplicateIndex !== -1) {
    errorEl.textContent = 'A script with this name already exists';
    return;
  }

  const scriptData = {
    name,
    trigger: trigger || null,
    code,
    enabled: true
  };

  if (editingScriptIndex !== null) {
    // Update existing script
    scripts[editingScriptIndex] = scriptData;
    appendOutput(`✓ Script "${name}" updated`, 'system');
  } else {
    // Create new script
    scripts.push(scriptData);
    appendOutput(`✓ Script "${name}" created`, 'system');
  }

  saveScripts();
  renderScriptsList();
  hideScriptModal();
}

/**
 * Edit script
 */
function editScript(index) {
  showScriptModal(index);
}

/**
 * Delete script
 */
function deleteScript(index) {
  const script = scripts[index];

  if (confirm(`Delete script "${script.name}"?`)) {
    scripts.splice(index, 1);
    saveScripts();
    renderScriptsList();
    appendOutput(`✓ Script "${script.name}" deleted`, 'system');
  }
}

/**
 * Toggle script enabled state
 */
function toggleScriptEnabled(index) {
  scripts[index].enabled = !scripts[index].enabled;
  saveScripts();
  renderScriptsList();
}

/**
 * Execute script by index
 */
async function executeScriptByIndex(index) {
  const script = scripts[index];

  if (!script.enabled) {
    appendOutput(`⚠️ Script "${script.name}" is disabled`, 'system');
    return;
  }

  appendOutput(`▶️ Running script: ${script.name}`, 'system');
  await executeScript(script.code, script.name);
}

/**
 * Check if text matches any script triggers
 */
function checkScriptTriggers(text) {
  for (const script of scripts) {
    if (!script.enabled || !script.trigger) continue;

    try {
      const regex = new RegExp(script.trigger);
      if (regex.test(text)) {
        console.log(`⚡ Script trigger fired: "${script.name}"`);
        executeScript(script.code, script.name);
      }
    } catch (error) {
      console.error(`Invalid script trigger pattern in "${script.name}":`, error);
    }
  }
}

// ===========================
// Timer Functions
// ===========================

/**
 * Load timers from localStorage
 */
async function loadTimers() {
  try {
    const backendTimers = await invoke('list_timers');
    // Normalize timer data from backend format to frontend format
    timers = backendTimers.map(timer => normalizeTimer(timer));
    renderTimersList();
    // Restart any enabled timers
    timers.forEach((timer, index) => {
      if (timer.enabled) {
        startTimer(index);
      }
    });
  } catch (error) {
    console.error('Failed to load timers:', error);
    appendOutput(`❌ Failed to load timers: ${error}`, 'error');
  }
}

/**
 * Normalize timer from backend format to frontend format
 */
function normalizeTimer(timer) {
  // Convert interval from Duration object to seconds
  const interval = timer.interval.secs || timer.interval;

  // Extract script from action enum
  let script = '';
  if (timer.action && timer.action.ExecuteScript) {
    script = timer.action.ExecuteScript;
  }

  return {
    ...timer,
    interval,
    script
  };
}

/**
 * Start a timer
 */
function startTimer(index) {
  const timer = timers[index];
  if (!timer || !timer.enabled) return;

  // Stop existing timer if running
  stopTimer(index);

  const timerId = `timer-${index}`;
  const intervalMs = timer.interval * 1000;

  // Set up recurring execution
  const intervalHandle = setInterval(() => {
    executeTimerAction(timer);

    // Update next execution time
    timerNextExecution.set(timerId, Date.now() + intervalMs);
  }, intervalMs);

  activeTimerIntervals.set(timerId, intervalHandle);
  timerNextExecution.set(timerId, Date.now() + intervalMs);

  console.log(`⏰ Timer started: "${timer.name}" (every ${timer.interval}s)`);
}

/**
 * Stop a timer
 */
function stopTimer(index) {
  const timerId = `timer-${index}`;

  if (activeTimerIntervals.has(timerId)) {
    clearInterval(activeTimerIntervals.get(timerId));
    activeTimerIntervals.delete(timerId);
    timerNextExecution.delete(timerId);
    console.log(`⏰ Timer stopped: "${timers[index]?.name}"`);
  }
}

/**
 * Execute timer action (Lua script)
 */
async function executeTimerAction(timer) {
  try {
    // Execute the timer's Lua script
    appendOutput(`⏰ Timer executing: ${timer.name}`, 'system');
    await executeScript(timer.script, `Timer: ${timer.name}`);
  } catch (error) {
    console.error(`Timer execution error in "${timer.name}":`, error);
    appendOutput(`❌ Timer error in "${timer.name}": ${error.message}`, 'error');
  }
}

/**
 * Start all enabled timers
 */
function startAllTimers() {
  timers.forEach((timer, index) => {
    if (timer.enabled) {
      startTimer(index);
    }
  });
}

/**
 * Stop all timers
 */
function stopAllTimers() {
  activeTimerIntervals.forEach((intervalHandle, timerId) => {
    clearInterval(intervalHandle);
  });
  activeTimerIntervals.clear();
  timerNextExecution.clear();
  console.log('⏰ All timers stopped');
}

/**
 * Render timers list
 */
function renderTimersList() {
  const timerList = document.getElementById('timer-list');

  if (timers.length === 0) {
    timerList.innerHTML = '<p class="empty-state">No timers yet</p>';
    return;
  }

  timerList.innerHTML = timers.map((timer, index) => {
    const timerId = `timer-${index}`;
    const isRunning = activeTimerIntervals.has(timerId);
    const nextExec = timerNextExecution.get(timerId);
    const timeRemaining = nextExec ? Math.max(0, Math.ceil((nextExec - Date.now()) / 1000)) : 0;

    return `
      <div class="timer-item ${timer.enabled ? '' : 'disabled'} ${isRunning ? 'running' : ''}">
        <div class="timer-header">
          <label class="timer-checkbox">
            <input type="checkbox" ${timer.enabled ? 'checked' : ''}
                   onchange="toggleTimerEnabled(${index})" />
            <span class="timer-name">${escapeHtml(timer.name)}</span>
          </label>
          <div class="timer-actions">
            <button class="btn btn-icon btn-sm" onclick="editTimer(${index})" title="Edit Timer">✏️</button>
            <button class="btn btn-icon btn-sm btn-danger" onclick="deleteTimer(${index})" title="Delete Timer">🗑️</button>
          </div>
        </div>
        <div class="timer-info">
          <span class="timer-interval">Every ${timer.interval}s</span>
          ${isRunning ? `<span class="timer-countdown">Next in ${timeRemaining}s</span>` : ''}
          <span class="timer-type">⚙️ Lua Script</span>
        </div>
      </div>
    `;
  }).join('');
}

/**
 * Update timer countdowns in UI
 */
function updateTimerCountdowns() {
  timers.forEach((timer, index) => {
    if (!timer.enabled) return;

    const timerId = `timer-${index}`;
    const nextExec = timerNextExecution.get(timerId);

    if (nextExec) {
      const timeRemaining = Math.max(0, Math.ceil((nextExec - Date.now()) / 1000));
      const countdownEl = document.querySelector(`#timer-list .timer-item:nth-child(${index + 1}) .timer-countdown`);
      if (countdownEl) {
        countdownEl.textContent = `Next in ${timeRemaining}s`;
      }
    }
  });
}

/**
 * Show timer modal for creating/editing
 */
function showTimerModal(index = null) {
  editingTimerIndex = index;
  const modal = document.getElementById('timer-modal');
  const form = document.getElementById('timer-form');
  const title = document.querySelector('#timer-modal .modal-header h2');
  const submitBtn = document.querySelector('#timer-form button[type="submit"]');

  if (index !== null) {
    // Edit mode
    const timer = timers[index];
    title.textContent = 'Edit Timer';
    submitBtn.textContent = 'Update Timer';
    document.getElementById('timer-name').value = timer.name;
    document.getElementById('timer-interval').value = timer.interval;
    document.getElementById('timer-script').value = timer.script || '';
  } else {
    // Create mode
    title.textContent = 'Create Timer';
    submitBtn.textContent = 'Create Timer';
    form.reset();
  }

  document.getElementById('timer-error').textContent = '';
  modal.style.display = 'flex';
}

/**
 * Hide timer modal
 */
function hideTimerModal() {
  document.getElementById('timer-modal').style.display = 'none';
  editingTimerIndex = null;
}

/**
 * Handle create/edit timer form submission
 */
async function handleCreateTimer(event) {
  event.preventDefault();

  const name = document.getElementById('timer-name').value.trim();
  const interval = parseInt(document.getElementById('timer-interval').value);
  const script = document.getElementById('timer-script').value;
  const errorEl = document.getElementById('timer-error');

  if (!name) {
    errorEl.textContent = 'Timer name is required';
    return;
  }

  if (!interval || interval < 1) {
    errorEl.textContent = 'Interval must be at least 1 second';
    return;
  }

  if (!script || !script.trim()) {
    errorEl.textContent = 'Lua script is required';
    return;
  }

  try {
    if (editingTimerIndex !== null) {
      // Update existing timer
      const timer = timers[editingTimerIndex];
      await invoke('update_timer', {
        request: {
          id: timer.id,
          name,
          timer_type: 'repeating',
          interval_secs: interval,
          action: 'execute_script',
          script,
          enabled: timer.enabled
        }
      });

      appendOutput(`✓ Timer "${name}" updated`, 'system');
    } else {
      // Create new timer
      await invoke('create_timer', {
        request: {
          name,
          timer_type: 'repeating',
          interval_secs: interval,
          action: 'execute_script',
          script,
          enabled: true
        }
      });

      appendOutput(`✓ Timer "${name}" created`, 'system');
    }

    // Reload timers from backend
    await loadTimers();
    hideTimerModal();
  } catch (error) {
    console.error('Failed to save timer:', error);
    errorEl.textContent = `Error: ${error}`;
  }
}

/**
 * Edit timer
 */
function editTimer(index) {
  showTimerModal(index);
}

/**
 * Delete timer
 */
async function deleteTimer(index) {
  const timer = timers[index];

  if (confirm(`Delete timer "${timer.name}"?`)) {
    try {
      stopTimer(index);
      await invoke('delete_timer', { id: timer.id });
      await loadTimers();
      appendOutput(`✓ Timer "${timer.name}" deleted`, 'system');
    } catch (error) {
      console.error('Failed to delete timer:', error);
      appendOutput(`❌ Failed to delete timer: ${error}`, 'error');
    }
  }
}

/**
 * Toggle timer enabled state
 */
async function toggleTimerEnabled(index) {
  const timer = timers[index];
  const newEnabled = !timer.enabled;

  try {
    await invoke('update_timer', {
      request: {
        id: timer.id,
        enabled: newEnabled
      }
    });

    if (newEnabled) {
      timers[index].enabled = true;
      startTimer(index);
    } else {
      timers[index].enabled = false;
      stopTimer(index);
    }

    renderTimersList();
  } catch (error) {
    console.error('Failed to toggle timer:', error);
    appendOutput(`❌ Failed to toggle timer: ${error}`, 'error');
  }
}

/**
 * Handle connection form submission
 */
async function handleConnect(event) {
  event.preventDefault();

  const name = document.getElementById('world-name').value;
  const host = document.getElementById('host').value;
  const port = parseInt(document.getElementById('port').value);
  const timeout = parseInt(document.getElementById('timeout').value) || 30;
  const useTls = document.getElementById('use-tls').checked;
  const autoReconnect = document.getElementById('auto-reconnect').checked;
  const keepAlive = document.getElementById('keep-alive').checked;

  connectError.textContent = '';

  try {
    // Create new world object
    const world = createWorld(name || `${host}:${port}`, host, port, useTls);
    world.autoReconnectEnabled = autoReconnect;

    // Make this the active world
    activeWorldId = world.id;

    // Call Rust backend to connect
    const result = await invoke('connect_to_world', {
      request: { name: world.name, host, port, use_tls: useTls }
    });

    if (result.connected) {
      world.isConnected = true;
      world.connectionStartTime = Date.now();

      clearOutput();
      appendOutput(`=== Connected to ${result.world_name || world.name} ===`, 'system');
      appendOutput(`Host: ${host}:${port}`, 'system');

      // Display loaded automation counts
      const automationCounts = [];
      if (result.triggers_loaded > 0) {
        automationCounts.push(`${result.triggers_loaded} triggers`);
      }
      if (result.aliases_loaded > 0) {
        automationCounts.push(`${result.aliases_loaded} aliases`);
      }
      if (result.timers_loaded > 0) {
        automationCounts.push(`${result.timers_loaded} timers`);
      }
      if (result.highlights_loaded > 0) {
        automationCounts.push(`${result.highlights_loaded} highlights`);
      }

      if (automationCounts.length > 0) {
        appendOutput(`📦 Loaded: ${automationCounts.join(', ')}`, 'system');
      } else {
        appendOutput('📦 No saved automation loaded', 'system');
      }

      // Show active settings
      if (autoReconnect) {
        appendOutput('⚙️ Auto-reconnect: enabled', 'system');
      }
      if (keepAlive) {
        appendOutput('⚙️ Keep-alive: enabled', 'system');
      }
      appendOutput('', 'system');

      updateConnectionStatus(true, result.world_name);

      // Add to recent connections
      addRecentConnection(world.name, host, port);

      // Start keep-alive if enabled
      if (keepAlive) {
        world.keepAliveInterval = setInterval(() => {
          // Keep-alive logic
        }, 60000);
      }

      // Start status bar
      startStatusBar();

      // Start all enabled timers
      startAllTimers();

      // Update world tabs
      renderWorldTabs();

      // Save worlds state
      saveWorlds();

      // Hide connection dialog
      connectionDialog.style.display = 'none';
    }
  } catch (error) {
    connectError.textContent = `Connection failed: ${error}`;
    console.error('Connection error:', error);
  }
}

/**
 * Handle disconnect
 */
async function handleDisconnect() {
  try {
    // Stop keep-alive timer
    stopKeepAlive();

    // Stop status bar
    stopStatusBar();

    // Stop all timers
    stopAllTimers();

    // Disable auto-reconnect on manual disconnect
    autoReconnectEnabled = false;

    await invoke('disconnect');
    appendOutput('', 'system');
    appendOutput('=== Disconnected ===', 'system');
    updateConnectionStatus(false);
  } catch (error) {
    console.error('Disconnect error:', error);
    appendOutput(`Disconnect error: ${error}`, 'error');
  }
}

/**
 * Start keep-alive timer
 */
function startKeepAlive() {
  // Clear any existing timer
  stopKeepAlive();

  // Send a space character every 60 seconds to prevent timeout
  keepAliveInterval = setInterval(async () => {
    if (isConnected) {
      try {
        // Send a single space - most MUDs ignore this
        await invoke('send_command', { command: ' ' });
        console.log('Keep-alive ping sent');
      } catch (error) {
        console.error('Keep-alive error:', error);
        stopKeepAlive();
      }
    } else {
      stopKeepAlive();
    }
  }, 60000); // 60 seconds

  console.log('Keep-alive started (60s interval)');
}

/**
 * Stop keep-alive timer
 */
function stopKeepAlive() {
  if (keepAliveInterval) {
    clearInterval(keepAliveInterval);
    keepAliveInterval = null;
    console.log('Keep-alive stopped');
  }
}

/**
 * Attempt to reconnect with last connection settings
 */
async function attemptReconnect() {
  if (!lastConnectionSettings || !autoReconnectEnabled) {
    return;
  }

  appendOutput('', 'system');
  appendOutput('🔄 Auto-reconnect enabled, attempting to reconnect...', 'system');

  // Wait 3 seconds before reconnecting
  await new Promise(resolve => setTimeout(resolve, 3000));

  // Check if still disconnected
  if (!isConnected && autoReconnectEnabled) {
    // Populate form with last settings
    document.getElementById('world-name').value = lastConnectionSettings.name;
    document.getElementById('host').value = lastConnectionSettings.host;
    document.getElementById('port').value = lastConnectionSettings.port;
    document.getElementById('timeout').value = lastConnectionSettings.timeout;
    document.getElementById('auto-reconnect').checked = lastConnectionSettings.autoReconnect;
    document.getElementById('keep-alive').checked = lastConnectionSettings.keepAlive;

    // Trigger connection
    try {
      await handleConnect({ preventDefault: () => {} });
    } catch (error) {
      appendOutput(`⚠️ Auto-reconnect failed: ${error}`, 'error');
      appendOutput('💡 Reconnection will not be attempted again', 'system');
      autoReconnectEnabled = false;
    }
  }
}

/**
 * Send command to MUD server
 */
async function sendCommand(command) {
  if (!command.trim()) return;

  // Process through aliases
  const processedCommand = processAliases(command);

  // Show original command in output
  appendOutput(`> ${command}`, 'command');

  // Show if alias was applied
  if (processedCommand !== command) {
    appendOutput(`  → ${processedCommand}`, 'system');
  }

  // Add to history (store original command)
  addToCommandHistory(command);

  try {
    // Track bytes sent
    trackCommandSent(processedCommand);

    await invoke('send_command', { command: processedCommand });
  } catch (error) {
    appendOutput(`Error: ${error}`, 'error');
    console.error('Send command error:', error);
  }
}

/**
 * Handle command form submission
 */
async function handleCommandSubmit(event) {
  event.preventDefault();

  const command = commandInput.value;
  if (!command.trim()) return;

  await sendCommand(command);
  commandInput.value = '';
}

/**
 * Handle command input keyboard events
 */
/**
 * Handle tab-completion
 */
async function handleTabCompletion() {
  const world = getActiveWorld();
  if (!world) return;

  const input = commandInput.value;
  const cursorPos = commandInput.selectionStart;

  // Extract the partial word at cursor position
  const textBeforeCursor = input.substring(0, cursorPos);
  const lastSpaceIndex = textBeforeCursor.lastIndexOf(' ');
  const partial = lastSpaceIndex >= 0
    ? textBeforeCursor.substring(lastSpaceIndex + 1)
    : textBeforeCursor;

  // Check if we're continuing a tab-completion sequence
  if (!world.isTabCompleting || world.tabCompletionPartial !== partial) {
    // Start new tab-completion
    try {
      const matches = await invoke('get_tab_completions', { partial });

      if (matches.length === 0) {
        // No matches, beep or show indication
        return;
      }

      world.tabCompletionMatches = matches;
      world.tabCompletionIndex = 0;
      world.tabCompletionPartial = partial;
      world.isTabCompleting = true;
    } catch (error) {
      console.error('Tab-completion error:', error);
      return;
    }
  } else {
    // Cycle to next match
    world.tabCompletionIndex = (world.tabCompletionIndex + 1) % world.tabCompletionMatches.length;
  }

  // Apply completion
  const match = world.tabCompletionMatches[world.tabCompletionIndex];
  const textAfterCursor = input.substring(cursorPos);
  const textBeforePartial = lastSpaceIndex >= 0
    ? textBeforeCursor.substring(0, lastSpaceIndex + 1)
    : '';

  commandInput.value = textBeforePartial + match + textAfterCursor;
  commandInput.selectionStart = commandInput.selectionEnd = (textBeforePartial + match).length;

  // Show completion info if multiple matches
  if (world.tabCompletionMatches.length > 1) {
    const status = `[${world.tabCompletionIndex + 1}/${world.tabCompletionMatches.length}] ${world.tabCompletionMatches.join(', ')}`;
    appendOutput(status, 'system');
  }
}

/**
 * Reset tab-completion state
 */
function resetTabCompletion() {
  const world = getActiveWorld();
  if (world) {
    world.isTabCompleting = false;
    world.tabCompletionMatches = [];
    world.tabCompletionIndex = -1;
    world.tabCompletionPartial = '';
  }
}

function handleCommandInputKeydown(event) {
  // Handle TAB key for completion
  if (event.key === 'Tab') {
    event.preventDefault();
    handleTabCompletion();
    return;
  }

  // Reset tab-completion on any other key
  if (event.key !== 'Tab') {
    resetTabCompletion();
  }

  // Handle history search mode
  if (isSearchingHistory) {
    if (event.key === 'Escape') {
      event.preventDefault();
      exitHistorySearch(false);
    } else if (event.key === 'Enter') {
      event.preventDefault();
      exitHistorySearch(true);
    } else if (event.key === 'ArrowUp' || (event.ctrlKey && event.key === 'r')) {
      event.preventDefault();
      nextHistorySearchMatch();
    } else if (event.key === 'ArrowDown') {
      event.preventDefault();
      // No-op in search mode
    }
    return;
  }

  // Start history search (Ctrl+R)
  if (event.ctrlKey && event.key === 'r') {
    event.preventDefault();
    startHistorySearch();
    return;
  }

  // Navigate history with arrow keys
  if (event.key === 'ArrowUp') {
    event.preventDefault();
    navigateHistoryUp();
  } else if (event.key === 'ArrowDown') {
    event.preventDefault();
    navigateHistoryDown();
  } else if (event.key === 'Escape') {
    // Reset history navigation
    if (commandHistoryIndex !== -1) {
      event.preventDefault();
      commandInput.value = currentCommand;
      commandHistoryIndex = -1;
      currentCommand = '';
    }
  }
}

/**
 * Handle command input changes (for search mode)
 */
function handleCommandInputChange(event) {
  if (isSearchingHistory) {
    updateHistorySearch(event.target.value);
  }
}

/**
 * Check connection status on startup
 */
async function checkConnectionStatus() {
  try {
    const status = await invoke('get_connection_status');
    updateConnectionStatus(status.connected, status.world_name);

    if (status.connected) {
      appendOutput(`=== Reconnected to ${status.world_name} ===`, 'system');
    }
  } catch (error) {
    console.error('Error checking connection status:', error);
  }
}

/**
 * Check for auto-connect world and connect if found
 */
async function checkAutoConnect() {
  // Find favorite world
  const favoriteWorld = savedWorlds.find(w => w.autoConnect);

  if (favoriteWorld) {
    console.log(`Auto-connecting to favorite world: ${favoriteWorld.name}`);
    appendOutput(`🌟 Auto-connecting to "${favoriteWorld.name}"...`, 'system');

    // Populate form fields
    document.getElementById('world-name').value = favoriteWorld.name;
    document.getElementById('host').value = favoriteWorld.host;
    document.getElementById('port').value = favoriteWorld.port;
    document.getElementById('timeout').value = favoriteWorld.timeout || 30;
    document.getElementById('auto-reconnect').checked = favoriteWorld.autoReconnect || false;
    document.getElementById('keep-alive').checked = favoriteWorld.keepAlive !== false;

    // Trigger connection after a short delay
    setTimeout(() => {
      connectForm.dispatchEvent(new Event('submit'));
    }, 500);
  }
}

/**
 * Show trigger modal for creating or editing
 */
function showTriggerModal(triggerIndex = null) {
  editingTriggerIndex = triggerIndex;

  if (triggerIndex !== null) {
    // Editing existing trigger
    const trigger = triggers[triggerIndex];
    document.getElementById('trigger-name').value = trigger.name;
    document.getElementById('trigger-pattern').value = trigger.pattern;
    document.getElementById('trigger-command').value = trigger.command;
    document.querySelector('.modal-header h2').textContent = 'Edit Trigger';
    document.querySelector('#trigger-form button[type="submit"]').textContent = 'Update Trigger';
  } else {
    // Creating new trigger
    triggerForm.reset();
    document.querySelector('.modal-header h2').textContent = 'Create Trigger';
    document.querySelector('#trigger-form button[type="submit"]').textContent = 'Create Trigger';
  }

  triggerModal.style.display = 'flex';
  document.getElementById('trigger-name').focus();
}

/**
 * Hide trigger modal
 */
function hideTriggerModal() {
  triggerModal.style.display = 'none';
  triggerForm.reset();
  triggerError.textContent = '';
  editingTriggerIndex = null;
}

/**
 * Render trigger list
 */
function renderTriggerList() {
  if (triggers.length === 0) {
    triggerList.innerHTML = '<p class="empty-state">No triggers yet</p>';
    return;
  }

  triggerList.innerHTML = triggers.map((trigger, index) => {
    const enabled = trigger.enabled !== false; // Default to true if not set
    const disabledClass = enabled ? '' : 'disabled';
    const matchCount = trigger.match_count || 0;
    return `
    <div class="trigger-item ${disabledClass}" data-index="${index}">
      <div class="trigger-item-header">
        <div class="trigger-header-left">
          <label class="trigger-toggle">
            <input type="checkbox" class="toggle-trigger" data-index="${index}" ${enabled ? 'checked' : ''}>
            <span class="toggle-slider"></span>
          </label>
          <span class="trigger-name">${trigger.name}</span>
          <span class="trigger-match-count" title="Matches">${matchCount > 0 ? `🎯 ${matchCount}` : ''}</span>
        </div>
        <div class="trigger-actions">
          <button class="test-trigger" data-index="${index}">Test</button>
          <button class="edit-trigger" data-index="${index}">Edit</button>
          <button class="delete-trigger" data-index="${index}">Delete</button>
        </div>
      </div>
      <div class="trigger-pattern">Pattern: ${trigger.pattern}</div>
      <div class="trigger-command">→ ${trigger.command}</div>
    </div>
  `;
  }).join('');

  // Add event listeners
  document.querySelectorAll('.toggle-trigger').forEach(checkbox => {
    checkbox.addEventListener('change', handleToggleTrigger);
  });
  document.querySelectorAll('.test-trigger').forEach(btn => {
    btn.addEventListener('click', handleTestTrigger);
  });
  document.querySelectorAll('.edit-trigger').forEach(btn => {
    btn.addEventListener('click', handleEditTrigger);
  });
  document.querySelectorAll('.delete-trigger').forEach(btn => {
    btn.addEventListener('click', handleDeleteTrigger);
  });
}

/**
 * Render alias list
 */
function renderAliasList() {
  if (!aliasList) return;

  if (aliases.length === 0) {
    aliasList.innerHTML = '<p class="empty-state">No aliases yet</p>';
    return;
  }

  aliasList.innerHTML = aliases.map((alias, index) => {
    const enabled = alias.enabled !== false; // Default to true if not set
    const disabledClass = enabled ? '' : 'disabled';
    return `
    <div class="alias-item ${disabledClass}" data-index="${index}">
      <div class="alias-item-header">
        <div class="alias-item-name">
          <label class="trigger-toggle">
            <input type="checkbox" class="toggle-alias" data-index="${index}" ${enabled ? 'checked' : ''}>
            <span class="toggle-slider"></span>
          </label>
          ${alias.name}
        </div>
        <div class="alias-item-actions">
          <button class="btn-icon test-alias" data-index="${index}" title="Test">🧪</button>
          <button class="btn-icon edit-alias" data-index="${index}" title="Edit">✏️</button>
          <button class="btn-icon delete-alias" data-index="${index}" title="Delete">🗑️</button>
        </div>
      </div>
      <div class="alias-item-pattern">${alias.pattern}</div>
      <div class="alias-item-replacement">→ ${alias.replacement}</div>
    </div>
  `;
  }).join('');

  // Add event listeners
  document.querySelectorAll('.toggle-alias').forEach(checkbox => {
    checkbox.addEventListener('change', handleToggleAlias);
  });
  document.querySelectorAll('.test-alias').forEach(btn => {
    btn.addEventListener('click', handleTestAlias);
  });
  document.querySelectorAll('.edit-alias').forEach(btn => {
    btn.addEventListener('click', handleEditAlias);
  });
  document.querySelectorAll('.delete-alias').forEach(btn => {
    btn.addEventListener('click', handleDeleteAlias);
  });
}

/**
 * Show alias modal
 */
function showAliasModal(aliasIndex = null) {
  editingAliasIndex = aliasIndex;

  if (aliasIndex !== null) {
    // Editing existing alias
    const alias = aliases[aliasIndex];
    document.getElementById('alias-name').value = alias.name;
    document.getElementById('alias-pattern').value = alias.pattern;
    document.getElementById('alias-replacement').value = alias.replacement;
    document.querySelector('#alias-modal .modal-header h2').textContent = 'Edit Alias';
    document.querySelector('#alias-form button[type="submit"]').textContent = 'Update Alias';
  } else {
    // Creating new alias
    aliasForm.reset();
    document.querySelector('#alias-modal .modal-header h2').textContent = 'Create Alias';
    document.querySelector('#alias-form button[type="submit"]').textContent = 'Create Alias';
  }

  aliasModal.style.display = 'flex';
  document.getElementById('alias-name').focus();
}

/**
 * Hide alias modal
 */
function hideAliasModal() {
  aliasModal.style.display = 'none';
  aliasForm.reset();
  aliasError.textContent = '';
  editingAliasIndex = null;
}

/**
 * Handle alias form submission (create or update)
 */
async function handleCreateAlias(event) {
  event.preventDefault();

  const name = document.getElementById('alias-name').value;
  const pattern = document.getElementById('alias-pattern').value;
  const replacement = document.getElementById('alias-replacement').value;
  const script = document.getElementById('alias-script').value;

  aliasError.textContent = '';

  try {
    // Test that pattern is valid regex
    new RegExp(pattern);

    // Determine action type
    const action = script ? 'execute_script' : 'send_command';
    const command = script || replacement;

    if (editingAliasIndex !== null) {
      // Update existing alias
      const alias = aliases[editingAliasIndex];
      await invoke('update_alias', {
        request: {
          id: alias.id,
          name,
          pattern,
          action,
          [script ? 'script' : 'command']: command,
          enabled: alias.enabled
        }
      });
      appendOutput(`✓ Alias updated: "${name}"`, 'system');
    } else {
      // Create new alias
      await invoke('create_alias', {
        request: {
          name,
          pattern,
          action,
          [script ? 'script' : 'command']: command,
          enabled: true
        }
      });
      appendOutput(`✓ Alias created: "${name}"`, 'system');
    }

    // Reload aliases from backend
    await loadAliases();
    hideAliasModal();
  } catch (error) {
    console.error('Failed to save alias:', error);
    aliasError.textContent = `Error: ${error}`;
  }
}

/**
 * Handle edit alias button click
 */
function handleEditAlias(event) {
  const index = parseInt(event.target.closest('.btn-icon').dataset.index);
  showAliasModal(index);
}

/**
 * Handle delete alias button click
 */
async function handleDeleteAlias(event) {
  const index = parseInt(event.target.closest('.btn-icon').dataset.index);
  const alias = aliases[index];

  if (confirm(`Delete alias "${alias.name}"?`)) {
    try {
      await invoke('delete_alias', { id: alias.id });
      await loadAliases();
      appendOutput(`✓ Alias deleted: "${alias.name}"`, 'system');
    } catch (error) {
      console.error('Failed to delete alias:', error);
      appendOutput(`❌ Failed to delete alias: ${error}`, 'error');
    }
  }
}

/**
 * Handle toggle alias enabled/disabled
 */
async function handleToggleAlias(event) {
  const index = parseInt(event.target.dataset.index);
  const alias = aliases[index];
  const newEnabled = event.target.checked;

  try {
    await invoke('update_alias', {
      request: {
        id: alias.id,
        enabled: newEnabled
      }
    });

    aliases[index].enabled = newEnabled;
    renderAliasList();

    const status = newEnabled ? 'enabled' : 'disabled';
    appendOutput(`✓ Alias "${alias.name}" ${status}`, 'system');
  } catch (error) {
    console.error('Failed to toggle alias:', error);
    appendOutput(`❌ Failed to toggle alias: ${error}`, 'error');
    // Revert checkbox state
    event.target.checked = !newEnabled;
  }
}

/**
 * Show test alias modal
 */
function showTestAliasModal(aliasIndex) {
  testingAliasIndex = aliasIndex;
  const alias = aliases[aliasIndex];

  document.getElementById('test-alias-name-display').value = alias.name;
  document.getElementById('test-alias-pattern-display').value = alias.pattern;
  document.getElementById('test-alias-input').value = '';
  testAliasResult.textContent = '';
  testAliasResult.className = 'test-result';

  testAliasModal.style.display = 'flex';
  document.getElementById('test-alias-input').focus();
}

/**
 * Hide test alias modal
 */
function hideTestAliasModal() {
  testAliasModal.style.display = 'none';
  testingAliasIndex = null;
}

/**
 * Handle test alias button click
 */
function handleTestAlias(event) {
  const index = parseInt(event.target.closest('.btn-icon').dataset.index);
  showTestAliasModal(index);
}

/**
 * Handle test alias form submission
 */
function handleTestAliasSubmit(event) {
  event.preventDefault();

  if (testingAliasIndex === null) return;

  const alias = aliases[testingAliasIndex];
  const testInput = document.getElementById('test-alias-input').value;

  try {
    const regex = new RegExp(alias.pattern);
    const match = testInput.match(regex);

    if (match) {
      // Apply replacement with capture groups
      let result = alias.replacement;
      for (let i = 1; i < match.length; i++) {
        result = result.replace(new RegExp(`\\$${i}`, 'g'), match[i]);
      }

      testAliasResult.className = 'test-result success';
      testAliasResult.innerHTML = `
        <strong>✅ Match!</strong><br>
        Command: <code>${testInput}</code><br>
        Will send: <code>${result}</code>
      `;

      if (match.length > 1) {
        testAliasResult.innerHTML += '<br><br><strong>Captured groups:</strong><br>';
        for (let i = 1; i < match.length; i++) {
          testAliasResult.innerHTML += `$${i} = "${match[i]}"<br>`;
        }
      }
    } else {
      testAliasResult.className = 'test-result error';
      testAliasResult.innerHTML = '<strong>❌ No match</strong><br>This command will not trigger the alias.';
    }
  } catch (error) {
    testAliasResult.className = 'test-result error';
    testAliasResult.textContent = `Error: ${error.message}`;
  }
}

/**
 * Process command through aliases
 */
function processAliases(command) {
  // Check each enabled alias
  for (const alias of aliases) {
    if (alias.enabled === false) continue;

    try {
      const regex = new RegExp(alias.pattern);
      const match = command.match(regex);

      if (match) {
        // Apply replacement with capture groups
        let result = alias.replacement;
        for (let i = 1; i < match.length; i++) {
          result = result.replace(new RegExp(`\\$${i}`, 'g'), match[i]);
        }
        return result;
      }
    } catch (error) {
      console.error(`Error in alias "${alias.name}":`, error);
    }
  }

  // No alias matched, return original command
  return command;
}

// ============================================================================
// MACRO UI MANAGEMENT
// ============================================================================

/**
 * Render macro list in sidebar
 */
function renderMacroList() {
  const macroKeys = Object.keys(macros);

  if (macroKeys.length === 0) {
    macroList.innerHTML = '<p class="empty-state">No macros yet</p>';
    return;
  }

  macroList.innerHTML = macroKeys
    .sort()
    .map((keyCombo) => {
      const command = macros[keyCombo];
      return `
        <div class="item">
          <div class="item-header">
            <span class="item-name" title="${command}">${keyCombo}</span>
            <div class="item-actions">
              <button class="btn-icon edit-macro" data-key="${keyCombo}" title="Edit">✏️</button>
              <button class="btn-icon delete-macro" data-key="${keyCombo}" title="Delete">🗑️</button>
            </div>
          </div>
          <div class="item-details">
            <small>${command}</small>
          </div>
        </div>
      `;
    })
    .join('');

  // Add event listeners
  macroList.querySelectorAll('.edit-macro').forEach((btn) => {
    btn.addEventListener('click', handleEditMacro);
  });

  macroList.querySelectorAll('.delete-macro').forEach((btn) => {
    btn.addEventListener('click', handleDeleteMacro);
  });
}

/**
 * Show macro modal for creating or editing
 */
function showMacroModal(keyCombo = null) {
  editingMacroKey = keyCombo;

  // Reset form
  document.getElementById('macro-name').value = '';
  document.getElementById('macro-key').value = '';
  document.getElementById('macro-command').value = '';
  macroError.textContent = '';

  if (keyCombo && macros[keyCombo]) {
    // Editing existing macro
    document.getElementById('macro-name').value = keyCombo;
    document.getElementById('macro-key').value = keyCombo;
    document.getElementById('macro-command').value = macros[keyCombo];
    document.querySelector('#macro-modal h2').textContent = 'Edit Macro';
    document.querySelector('#macro-form button[type="submit"]').textContent = 'Update Macro';
  } else {
    // Creating new macro
    document.querySelector('#macro-modal h2').textContent = 'Create Macro';
    document.querySelector('#macro-form button[type="submit"]').textContent = 'Create Macro';
  }

  macroModal.style.display = 'flex';

  // Focus on key input for key capture
  setTimeout(() => {
    macroKeyInput.focus();
  }, 100);
}

/**
 * Hide macro modal
 */
function hideMacroModal() {
  macroModal.style.display = 'none';
  editingMacroKey = null;
}

/**
 * Handle macro form submission
 */
function handleCreateMacro(event) {
  event.preventDefault();

  const keyCombo = document.getElementById('macro-key').value;
  const command = document.getElementById('macro-command').value;

  macroError.textContent = '';

  // Validation
  if (!keyCombo || !command) {
    macroError.textContent = 'Please fill in all fields';
    return;
  }

  // Check if key combo already exists (and not editing)
  if (macros[keyCombo] && editingMacroKey !== keyCombo) {
    macroError.textContent = `Key combination "${keyCombo}" is already assigned`;
    return;
  }

  // Remove old key if editing and key changed
  if (editingMacroKey && editingMacroKey !== keyCombo) {
    delete macros[editingMacroKey];
  }

  // Add or update macro
  addMacro(keyCombo, command);

  // Update UI
  renderMacroList();
  hideMacroModal();

  appendOutput(`✓ Macro ${editingMacroKey ? 'updated' : 'created'}: ${keyCombo} → ${command}`, 'system');
}

/**
 * Handle edit macro button click
 */
function handleEditMacro(event) {
  const keyCombo = event.target.dataset.key;
  showMacroModal(keyCombo);
}

/**
 * Handle delete macro button click
 */
function handleDeleteMacro(event) {
  const keyCombo = event.target.dataset.key;

  if (confirm(`Delete macro "${keyCombo}"?`)) {
    removeMacro(keyCombo);
    renderMacroList();
    appendOutput(`✓ Macro deleted: ${keyCombo}`, 'system');
  }
}

/**
 * Capture key combination for macro
 */
function handleMacroKeyCapture(event) {
  event.preventDefault();

  // Ignore if not focused on key input
  if (event.target !== macroKeyInput) {
    return;
  }

  // Build key combination string
  const modifiers = [];
  if (event.ctrlKey) modifiers.push('Ctrl');
  if (event.altKey) modifiers.push('Alt');
  if (event.shiftKey) modifiers.push('Shift');

  // Get the key name
  let keyName = event.key;

  // Normalize key names
  if (keyName.length === 1) {
    keyName = keyName.toUpperCase();
  }

  // Only allow function keys, letters, numbers with modifiers
  const isFunctionKey = /^F([1-9]|1[0-2])$/.test(keyName);
  const isAlphanumeric = /^[A-Z0-9]$/.test(keyName);

  // F-keys can be used alone, others need at least one modifier
  if (!isFunctionKey && (!isAlphanumeric || modifiers.length === 0)) {
    return;
  }

  // Build combination string
  const combination = modifiers.length > 0
    ? `${modifiers.join('+')}+${keyName}`
    : keyName;

  // Set the value
  macroKeyInput.value = combination;
}

// ============================================================================
// HIGHLIGHT UI MANAGEMENT
// ============================================================================

/**
 * Render highlight list in sidebar
 */
function renderHighlightList() {
  if (highlights.length === 0) {
    highlightList.innerHTML = '<p class="empty-state">No highlights yet</p>';
    return;
  }

  highlightList.innerHTML = highlights
    .map((highlight, index) => {
      const previewStyle = `color: ${highlight.color || '#ffff00'}; ${highlight.bold ? 'font-weight: bold;' : ''} ${highlight.italic ? 'font-style: italic;' : ''} ${highlight.underline ? 'text-decoration: underline;' : ''}`;
      const variables = highlight.variables && highlight.variables.length > 0
        ? ` → ${highlight.variables.join(', ')}`
        : '';

      return `
        <div class="item ${highlight.enabled === false ? 'disabled' : ''}">
          <div class="item-header">
            <label class="checkbox-label">
              <input
                type="checkbox"
                class="toggle-highlight"
                data-index="${index}"
                ${highlight.enabled !== false ? 'checked' : ''}
              />
              <span class="item-name" title="${highlight.pattern}">${highlight.name}</span>
            </label>
            <div class="item-actions">
              <button class="btn-icon edit-highlight" data-index="${index}" title="Edit">✏️</button>
              <button class="btn-icon delete-highlight" data-index="${index}" title="Delete">🗑️</button>
            </div>
          </div>
          <div class="item-details">
            <small><span style="${previewStyle}">Preview</span>${variables}</small>
          </div>
        </div>
      `;
    })
    .join('');

  // Add event listeners
  highlightList.querySelectorAll('.toggle-highlight').forEach((checkbox) => {
    checkbox.addEventListener('change', handleToggleHighlight);
  });

  highlightList.querySelectorAll('.edit-highlight').forEach((btn) => {
    btn.addEventListener('click', handleEditHighlight);
  });

  highlightList.querySelectorAll('.delete-highlight').forEach((btn) => {
    btn.addEventListener('click', handleDeleteHighlight);
  });
}

/**
 * Render variables list
 */
function renderVariablesList() {
  const varKeys = Object.keys(variables);

  if (varKeys.length === 0) {
    variablesList.innerHTML = '<p class="empty-state">No variables captured</p>';
    return;
  }

  variablesList.innerHTML = varKeys
    .sort()
    .map((key) => {
      const value = variables[key];
      return `
        <div class="variable-item">
          <span class="variable-name">${key}</span>
          <span class="variable-value">${value}</span>
        </div>
      `;
    })
    .join('');
}

/**
 * Show highlight modal for creating or editing
 */
function showHighlightModal(index = null) {
  editingHighlightIndex = index;

  // Reset form
  document.getElementById('highlight-name').value = '';
  document.getElementById('highlight-pattern').value = '';
  document.getElementById('highlight-color').value = '#ffff00';
  document.getElementById('highlight-color-text').value = '#ffff00';
  document.getElementById('highlight-bold').checked = false;
  document.getElementById('highlight-italic').checked = false;
  document.getElementById('highlight-underline').checked = false;
  document.getElementById('highlight-variables').value = '';
  highlightError.textContent = '';

  if (index !== null && highlights[index]) {
    // Editing existing highlight
    const highlight = highlights[index];
    document.getElementById('highlight-name').value = highlight.name;
    document.getElementById('highlight-pattern').value = highlight.pattern;
    document.getElementById('highlight-color').value = highlight.color || '#ffff00';
    document.getElementById('highlight-color-text').value = highlight.color || '#ffff00';
    document.getElementById('highlight-bold').checked = highlight.bold || false;
    document.getElementById('highlight-italic').checked = highlight.italic || false;
    document.getElementById('highlight-underline').checked = highlight.underline || false;
    document.getElementById('highlight-variables').value = highlight.variables ? highlight.variables.join(',') : '';
    document.querySelector('#highlight-modal h2').textContent = 'Edit Highlight';
    document.querySelector('#highlight-form button[type="submit"]').textContent = 'Update Highlight';
  } else {
    // Creating new highlight
    document.querySelector('#highlight-modal h2').textContent = 'Create Highlight';
    document.querySelector('#highlight-form button[type="submit"]').textContent = 'Create Highlight';
  }

  highlightModal.style.display = 'flex';
}

/**
 * Hide highlight modal
 */
function hideHighlightModal() {
  highlightModal.style.display = 'none';
  editingHighlightIndex = null;
}

/**
 * Handle highlight form submission
 */
async function handleCreateHighlight(event) {
  event.preventDefault();

  const name = document.getElementById('highlight-name').value;
  const pattern = document.getElementById('highlight-pattern').value;
  const color = document.getElementById('highlight-color').value;
  const bold = document.getElementById('highlight-bold').checked;
  const italic = document.getElementById('highlight-italic').checked;
  const underline = document.getElementById('highlight-underline').checked;
  const variablesInput = document.getElementById('highlight-variables').value;

  highlightError.textContent = '';

  // Validate regex
  try {
    new RegExp(pattern);
  } catch (error) {
    highlightError.textContent = `Invalid regex pattern: ${error.message}`;
    return;
  }

  // Parse variables
  const variables = variablesInput
    ? variablesInput.split(',').map(v => v.trim()).filter(v => v.length > 0)
    : [];

  try {
    if (editingHighlightIndex !== null) {
      // Update existing highlight
      const highlight = highlights[editingHighlightIndex];
      await invoke('update_highlight', {
        request: {
          id: highlight.id,
          name,
          pattern,
          color,
          bold,
          italic,
          underline,
          variables,
          enabled: highlight.enabled
        }
      });
      appendOutput(`✓ Highlight updated: ${name}`, 'system');
    } else {
      // Create new highlight
      await invoke('create_highlight', {
        request: {
          name,
          pattern,
          color,
          bold,
          italic,
          underline,
          variables,
          enabled: true
        }
      });
      appendOutput(`✓ Highlight created: ${name}`, 'system');
    }

    // Reload highlights from backend
    await loadHighlights();
    hideHighlightModal();
  } catch (error) {
    console.error('Failed to save highlight:', error);
    highlightError.textContent = `Error: ${error}`;
  }
}

/**
 * Handle edit highlight button click
 */
function handleEditHighlight(event) {
  const index = parseInt(event.target.dataset.index);
  showHighlightModal(index);
}

/**
 * Handle delete highlight button click
 */
async function handleDeleteHighlight(event) {
  const index = parseInt(event.target.dataset.index);
  const highlight = highlights[index];

  if (confirm(`Delete highlight "${highlight.name}"?`)) {
    try {
      await invoke('delete_highlight', { id: highlight.id });
      await loadHighlights();
      appendOutput(`✓ Highlight deleted: ${highlight.name}`, 'system');
    } catch (error) {
      console.error('Failed to delete highlight:', error);
      appendOutput(`❌ Failed to delete highlight: ${error}`, 'error');
    }
  }
}

/**
 * Handle toggle highlight checkbox
 */
async function handleToggleHighlight(event) {
  const index = parseInt(event.target.dataset.index);
  const highlight = highlights[index];
  const newEnabled = event.target.checked;

  try {
    await invoke('update_highlight', {
      request: {
        id: highlight.id,
        enabled: newEnabled
      }
    });

    highlights[index].enabled = newEnabled;
    renderHighlightList();
  } catch (error) {
    console.error('Failed to toggle highlight:', error);
    appendOutput(`❌ Failed to toggle highlight: ${error}`, 'error');
    // Revert checkbox state
    event.target.checked = !newEnabled;
  }
}

/**
 * Sync color picker with text input
 */
function syncColorInputs() {
  const colorPicker = document.getElementById('highlight-color');
  const colorText = document.getElementById('highlight-color-text');

  colorPicker.addEventListener('input', (e) => {
    colorText.value = e.target.value;
  });

  colorText.addEventListener('input', (e) => {
    if (/^#[0-9A-Fa-f]{6}$/.test(e.target.value)) {
      colorPicker.value = e.target.value;
    }
  });
}

/**
 * Handle trigger form submission (create or update)
 */
async function handleCreateTrigger(event) {
  event.preventDefault();

  const name = document.getElementById('trigger-name').value;
  const pattern = document.getElementById('trigger-pattern').value;
  const command = document.getElementById('trigger-command').value;
  const script = document.getElementById('trigger-script').value;

  triggerError.textContent = '';

  try {
    // Determine action type
    const action = script ? 'execute_script' : 'send_command';
    const actionField = script || command;

    if (editingTriggerIndex !== null) {
      // Update existing trigger
      const trigger = triggers[editingTriggerIndex];
      await invoke('update_trigger', {
        request: {
          id: trigger.id,
          name,
          pattern,
          action,
          [script ? 'script' : 'command']: actionField,
          enabled: trigger.enabled
        }
      });
      appendOutput(`✓ Trigger updated: "${name}"`, 'system');
    } else {
      // Create new trigger
      await invoke('create_trigger', {
        request: {
          name,
          pattern,
          action,
          [script ? 'script' : 'command']: actionField,
          enabled: true
        }
      });
      appendOutput(`✓ Trigger created: "${name}"`, 'system');
    }

    // Reload triggers from backend
    await loadTriggers();
    hideTriggerModal();
  } catch (error) {
    console.error('Failed to save trigger:', error);
    triggerError.textContent = `Failed to ${editingTriggerIndex !== null ? 'update' : 'create'} trigger: ${error}`;
    console.error('Trigger operation error:', error);
  }
}

/**
 * Handle toggle trigger enabled/disabled
 */
async function handleToggleTrigger(event) {
  const index = parseInt(event.target.dataset.index);
  const trigger = triggers[index];
  const newEnabled = event.target.checked;

  try {
    await invoke('update_trigger', {
      request: {
        id: trigger.id,
        enabled: newEnabled
      }
    });

    triggers[index].enabled = newEnabled;
    renderTriggerList();

    appendOutput(`${newEnabled ? '✓' : '⏸'} Trigger "${trigger.name}" ${newEnabled ? 'enabled' : 'disabled'}`, 'system');
  } catch (error) {
    console.error('Failed to toggle trigger:', error);
    appendOutput(`❌ Failed to toggle trigger: ${error}`, 'error');
    // Revert checkbox state
    event.target.checked = !newEnabled;
  }
}

/**
 * Handle edit trigger button click
 */
function handleEditTrigger(event) {
  const index = parseInt(event.target.dataset.index);
  showTriggerModal(index);
}

/**
 * Handle delete trigger
 */
async function handleDeleteTrigger(event) {
  const index = parseInt(event.target.dataset.index);
  const trigger = triggers[index];

  if (!confirm(`Delete trigger "${trigger.name}"?`)) {
    return;
  }

  try {
    await invoke('delete_trigger', { id: trigger.id });
    await loadTriggers();
    appendOutput(`✗ Trigger deleted: "${trigger.name}"`, 'system');
  } catch (error) {
    console.error('Failed to delete trigger:', error);
    appendOutput(`❌ Failed to delete trigger: ${error}`, 'error');
  }
}

/**
 * Show test trigger modal
 */
function showTestTriggerModal(triggerIndex) {
  testingTriggerIndex = triggerIndex;
  const trigger = triggers[triggerIndex];

  document.getElementById('test-trigger-name-display').value = trigger.name;
  document.getElementById('test-trigger-pattern-display').value = trigger.pattern;
  document.getElementById('test-input-text').value = '';
  testResult.textContent = '';
  testResult.className = 'test-result';

  testTriggerModal.style.display = 'flex';
  document.getElementById('test-input-text').focus();
}

/**
 * Hide test trigger modal
 */
function hideTestTriggerModal() {
  testTriggerModal.style.display = 'none';
  testingTriggerIndex = null;
}

/**
 * Handle test trigger button click
 */
function handleTestTrigger(event) {
  const index = parseInt(event.target.dataset.index);
  showTestTriggerModal(index);
}

/**
 * Handle test trigger form submission
 */
function handleTestTriggerSubmit(event) {
  event.preventDefault();

  const testInput = document.getElementById('test-input-text').value;
  const trigger = triggers[testingTriggerIndex];

  try {
    // Create a regex from the trigger pattern
    const regex = new RegExp(trigger.pattern);
    const isMatch = regex.test(testInput);

    if (isMatch) {
      testResult.textContent = `✓ MATCH! The trigger pattern matches this text.`;
      testResult.className = 'test-result match visible';
    } else {
      testResult.textContent = `✗ NO MATCH. The trigger pattern does not match this text.`;
      testResult.className = 'test-result no-match visible';
    }
  } catch (error) {
    testResult.textContent = `⚠️ ERROR: Invalid regex pattern - ${error.message}`;
    testResult.className = 'test-result no-match visible';
  }
}

/**
 * Handle events from backend
 */
function handleMudEvent(event) {
  const payload = event.payload;

  switch (payload.type) {
    case 'dataReceived':
      // Track bytes received
      trackDataReceived(payload.data || payload.text);

      // Apply backend highlight matches if available
      let displayText = payload.text;
      if (pendingHighlights.length > 0) {
        displayText = applyHighlightMatches(payload.text, pendingHighlights);
        pendingHighlights = []; // Clear after applying
      }

      // Check for trigger matches and update statistics
      const matched = checkTriggerMatches(payload.text);

      // Check for script triggers
      checkScriptTriggers(payload.text);

      // Display received text from MUD server
      // Add visual indicator if triggers matched
      appendOutput(displayText, matched ? 'trigger-matched' : '');
      break;

    case 'connectionStatus':
      // Update connection status
      updateConnectionStatus(payload.connected, payload.worldName);
      if (!payload.connected) {
        // Stop keep-alive
        stopKeepAlive();

        // Stop status bar
        stopStatusBar();

        // Stop all timers
        stopAllTimers();

        appendOutput('', 'system');
        appendOutput('=== Disconnected ===', 'system');

        // Attempt auto-reconnect if enabled
        if (autoReconnectEnabled) {
          attemptReconnect();
        }
      }
      break;

    case 'error':
      // Display error message
      appendOutput(`Error: ${payload.message}`, 'error');
      break;

    case 'highlightMatched':
      // Store highlight matches to apply to next dataReceived event
      pendingHighlights = payload.matches || [];
      console.debug(`Highlights matched: ${pendingHighlights.length} segments`);
      break;

    case 'triggerMatched':
      // Trigger matched incoming text - show notification
      appendOutput(`⚡ Trigger "${payload.triggerName}" matched`, 'system');
      console.debug(`Trigger matched: "${payload.triggerName}" on text: "${payload.matchedText}"`);
      break;

    case 'triggerExecuted':
      // Trigger executed commands
      appendOutput(`⚡ Trigger executed: ${payload.commands.length} command(s)`, 'system');
      break;

    case 'triggerError':
      // Trigger error
      appendOutput(`❌ Trigger error: ${payload.error}`, 'error');
      break;

    case 'aliasMatched':
      // Alias matched user input - show notification
      appendOutput(`🔀 Alias "${payload.aliasName}" matched`, 'system');
      console.debug(`Alias matched: "${payload.aliasName}" on input: "${payload.matchedText}"`);
      break;

    case 'aliasExecuted':
      // Alias executed commands
      appendOutput(`🔀 Alias expanded to: ${payload.commands.join('; ')}`, 'system');
      break;

    case 'aliasError':
      // Alias error
      appendOutput(`❌ Alias error: ${payload.error}`, 'error');
      break;

    case 'timerExecuted':
      // Timer executed commands
      appendOutput(`⏰ Timer fired: ${payload.commands.length} command(s)`, 'system');
      break;

    case 'timerError':
      // Timer error
      appendOutput(`❌ Timer error: ${payload.error}`, 'error');
      break;

    default:
      console.warn('Unknown event type:', payload.type);
  }
}

