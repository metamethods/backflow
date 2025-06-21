// WebSocket handler for sending input events to plumbershim
class WebSocketHandler {
  // todo: don't hardcode the ip
  constructor(url = "ws://192.168.1.34:8000") {
    this.url = url;
    this.ws = null;
    this.isConnected = false;
    this.reconnectAttempts = 0;
    this.maxReconnectAttempts = 5;
    this.reconnectDelay = 1000; // 1 second
    this.connect();
  }

  connect() {
    try {
      this.ws = new WebSocket(this.url);
      
      this.ws.onopen = () => {
        console.log("🟢 Connected to plumbershim WebSocket");
        this.isConnected = true;
        this.reconnectAttempts = 0;
      };

      this.ws.onmessage = (event) => {
        console.log("📥 Received from plumbershim:", event.data);
      };

      this.ws.onclose = (event) => {
        console.log("🔴 WebSocket connection closed");
        this.isConnected = false;
        this.attemptReconnect();
      };

      this.ws.onerror = (error) => {
        console.error("❌ WebSocket error:", error);
        this.isConnected = false;
      };
    } catch (error) {
      console.error("Failed to create WebSocket connection:", error);
      this.attemptReconnect();
    }
  }

  attemptReconnect() {
    if (this.reconnectAttempts < this.maxReconnectAttempts) {
      this.reconnectAttempts++;
      console.log(`🔄 Attempting to reconnect... (${this.reconnectAttempts}/${this.maxReconnectAttempts})`);
      setTimeout(() => {
        this.connect();
      }, this.reconnectDelay * this.reconnectAttempts);
    } else {
      console.error("Max reconnection attempts reached. Please refresh the page.");
    }
  }

  sendInputEvent(deviceId, events) {
    if (!this.isConnected || !this.ws) {
      console.warn("WebSocket not connected. Cannot send event.");
      return false;
    }

    const packet = {
      device_id: deviceId,
      timestamp: Date.now(),
      events: events
    };

    try {
      this.ws.send(JSON.stringify(packet));
      console.log("📤 Sent input packet:", packet);
      return true;
    } catch (error) {
      console.error("Failed to send WebSocket message:", error);
      return false;
    }
  }

  sendKeyboardEvent(key, pressed, deviceId = "chunitroller-webapp") {
    const eventType = pressed ? "KeyPress" : "KeyRelease";
    const events = [{
      "Keyboard": {
        [eventType]: {
          "key": key
        }
      }
    }];

    return this.sendInputEvent(deviceId, events);
  }

  sendPointerEvent(eventType, data, deviceId = "chunitroller-webapp") {
    const events = [{
      "Pointer": {
        [eventType]: data
      }
    }];

    return this.sendInputEvent(deviceId, events);
  }

  close() {
    if (this.ws) {
      this.ws.close();
      this.isConnected = false;
    }
  }
}

// this is a mess
class CellFeedbackHandler {
  constructor() {
    this.activeCells = new Set();
  }

  activateCell(cell) {
    if (!this.activeCells.has(cell)) {
      cell.style.backgroundColor = "white";
      cell.style.color = "black";
      this.activeCells.add(cell);
    }
  }

  deactivateCell(cell) {
    if (this.activeCells.has(cell)) {
      cell.style.backgroundColor = "";
      cell.style.color = "";
      this.activeCells.delete(cell);
    }
  }

  resetAll() {
    this.activeCells.forEach((cell) => {
      cell.style.backgroundColor = "";
      cell.style.color = "";
    });
    this.activeCells.clear();
  }
}
class GridController {
  constructor() {
    this.feedbackHandler = new CellFeedbackHandler();
    this.webSocketHandler = new WebSocketHandler();
    this.cells = [];
    this.activeTouches = new Map(); // touchId -> {cell, index}
    this.init();
  }

  init() {
    this.cells = document.querySelectorAll(".grid-cell");
    this.cells.forEach((cell, index) => {
      this.setupCellHandlers(cell, index);
    });
    this.setupGlobalTouchHandlers();

    console.log(`Grid controller initialized with ${this.cells.length} cells`);
  }

  setupCellHandlers(cell, index) {
    cell.addEventListener("touchstart", (e) => {
      e.preventDefault();
      Array.from(e.changedTouches).forEach((touch) => {
        if (!this.activeTouches.has(touch.identifier)) {
          this.activeTouches.set(touch.identifier, { cell, index });
          this.handleCellPress(cell, index);
        }
      });
    });
  }

  setupGlobalTouchHandlers() {
    document.addEventListener("touchmove", (e) => {
      if (this.activeTouches.size > 0) {
        e.preventDefault();
        this.handleTouchMove(e);
      }
    });

    document.addEventListener("touchend", (e) => {
      Array.from(e.changedTouches).forEach((touch) => {
        const touchData = this.activeTouches.get(touch.identifier);
        if (touchData) {
          this.feedbackHandler.deactivateCell(touchData.cell);
          
          // Send keyboard release event
          const key = this.mapCellToKey(touchData.cell);
          const deviceName = this.getDeviceNameForCell(touchData.cell);
          this.webSocketHandler.sendKeyboardEvent(key, false, deviceName);
          
          touchData.cell.dispatchEvent(
            new CustomEvent("cellrelease", {
              detail: {
                index: touchData.index,
                cell: touchData.cell,
                key: key,
                reason: "touch_end",
              },
              bubbles: true,
            }),
          );
          this.activeTouches.delete(touch.identifier);
        }
      });
    });

    document.addEventListener("touchcancel", (e) => {
      Array.from(e.changedTouches).forEach((touch) => {
        const touchData = this.activeTouches.get(touch.identifier);
        if (touchData) {
          this.feedbackHandler.deactivateCell(touchData.cell);
          
          // Send keyboard release event
          const key = this.mapCellToKey(touchData.cell);
          const deviceName = this.getDeviceNameForCell(touchData.cell);
          this.webSocketHandler.sendKeyboardEvent(key, false, deviceName);
          
          touchData.cell.dispatchEvent(
            new CustomEvent("cellrelease", {
              detail: {
                index: touchData.index,
                cell: touchData.cell,
                key: key,
                reason: "touch_cancel",
              },
              bubbles: true,
            }),
          );
          this.activeTouches.delete(touch.identifier);
        }
      });
    });
  }

  handleCellPress(cell, index) {
    this.feedbackHandler.activateCell(cell);
    
    // Send keyboard event for cell press
    const key = this.mapCellToKey(cell);
    const deviceName = this.getDeviceNameForCell(cell);
    this.webSocketHandler.sendKeyboardEvent(key, true, deviceName);
    
    cell.dispatchEvent(
      new CustomEvent("cellpress", {
        detail: { index, cell, key },
        bubbles: true,
      }),
    );
  }

  handleCellRelease(cell, index) {
    this.feedbackHandler.deactivateCell(cell);
    
    // Send keyboard event for cell release
    const key = this.mapCellToKey(cell);
    const deviceName = this.getDeviceNameForCell(cell);
    this.webSocketHandler.sendKeyboardEvent(key, false, deviceName);
    
    cell.dispatchEvent(
      new CustomEvent("cellrelease", {
        detail: { index, cell, key, reason: "normal" },
        bubbles: true,
      }),
    );
  }

  // Map cell to keyboard key using data-key attribute
  mapCellToKey(cellOrIndex) {
    let cell;
    if (typeof cellOrIndex === 'number') {
      // If given an index, get the cell element
      cell = this.cells[cellOrIndex];
    } else {
      // If given a cell element directly
      cell = cellOrIndex;
    }
    
    if (!cell) {
      console.warn("Could not find cell for mapping");
      return "KEY_UNKNOWN";
    }

    const dataKey = cell.getAttribute('data-key');
    if (!dataKey) {
      console.warn("Cell has no data-key attribute:", cell);
      return "KEY_UNKNOWN";
    }

    // Convert the data-key to Linux key code format
    return this.convertToLinuxKeyCode(dataKey);
  }

  // Convert data-key attribute to Linux key code format
  convertToLinuxKeyCode(dataKey) {
    // Handle special cases first
    const specialKeys = {
      'Backspace': 'KEY_BACKSPACE',
      'Tab': 'KEY_TAB',
      'Enter': 'KEY_ENTER',
      'CapsLock': 'KEY_CAPSLOCK',
      'ShiftLeft': 'KEY_LEFTSHIFT',
      'ShiftRight': 'KEY_RIGHTSHIFT',
      'ControlLeft': 'KEY_LEFTCTRL',
      'ControlRight': 'KEY_RIGHTCTRL',
      'AltLeft': 'KEY_LEFTALT',
      'AltRight': 'KEY_RIGHTALT',
      ' ': 'KEY_SPACE',
      '`': 'KEY_GRAVE',
      '-': 'KEY_MINUS',
      '=': 'KEY_EQUAL',
      '[': 'KEY_LEFTBRACE',
      ']': 'KEY_RIGHTBRACE',
      '\\': 'KEY_BACKSLASH',
      ';': 'KEY_SEMICOLON',
      "'": 'KEY_APOSTROPHE',
      ',': 'KEY_COMMA',
      '.': 'KEY_DOT',
      '/': 'KEY_SLASH'
    };

    if (specialKeys[dataKey]) {
      return specialKeys[dataKey];
    }

    // Handle numbers 0-9
    if (/^[0-9]$/.test(dataKey)) {
      return `KEY_${dataKey}`;
    }

    // Handle letters a-z (convert to uppercase for key codes)
    if (/^[a-zA-Z]$/.test(dataKey)) {
      return `KEY_${dataKey.toUpperCase()}`;
    }

    // If we don't recognize it, return as-is with KEY_ prefix
    console.warn("Unknown key mapping for:", dataKey);
    return `KEY_${dataKey.toUpperCase()}`;
  }

  // Get device name based on cell's section
  getDeviceNameForCell(cell) {
    // Find the parent container with data-cell-section
    let container = cell.closest('[data-cell-section]');
    if (container) {
      const section = container.getAttribute('data-cell-section');
      return `chunitroller-${section}`;
    }
    
    // Fallback to generic name
    return "chunitroller-webapp";
  }

  handleTouchMove(e) {
    Array.from(e.touches).forEach((touch) => {
      const touchData = this.activeTouches.get(touch.identifier);
      if (!touchData) return;

      const elementUnderTouch = document.elementFromPoint(
        touch.clientX,
        touch.clientY,
      );

      if (
        elementUnderTouch &&
        elementUnderTouch.classList.contains("grid-cell")
      ) {
        const newIndex = Array.from(this.cells).indexOf(elementUnderTouch);
        if (newIndex !== -1 && elementUnderTouch !== touchData.cell) {
          // Release the old cell
          this.feedbackHandler.deactivateCell(touchData.cell);
          const oldKey = this.mapCellToKey(touchData.cell);
          const oldDeviceName = this.getDeviceNameForCell(touchData.cell);
          this.webSocketHandler.sendKeyboardEvent(oldKey, false, oldDeviceName);
          
          touchData.cell.dispatchEvent(
            new CustomEvent("cellrelease", {
              detail: {
                index: touchData.index,
                cell: touchData.cell,
                key: oldKey,
                reason: "slid_away",
              },
              bubbles: true,
            }),
          );
          
          // Activate the new cell
          this.activeTouches.set(touch.identifier, {
            cell: elementUnderTouch,
            index: newIndex,
          });
          this.handleCellPress(elementUnderTouch, newIndex);
        }
      } else {
        // Moved outside the grid
        this.feedbackHandler.deactivateCell(touchData.cell);
        const key = this.mapCellToKey(touchData.cell);
        const deviceName = this.getDeviceNameForCell(touchData.cell);
        this.webSocketHandler.sendKeyboardEvent(key, false, deviceName);
        
        touchData.cell.dispatchEvent(
          new CustomEvent("cellrelease", {
            detail: {
              index: touchData.index,
              cell: touchData.cell,
              key: key,
              reason: "moved_outside",
            },
            bubbles: true,
          }),
        );
        this.activeTouches.delete(touch.identifier);
      }
    });
  }

  getCell(index) {
    return this.cells[index] || null;
  }

  getCellCount() {
    return this.cells.length;
  }

  setFeedbackHandler(newHandler) {
    this.feedbackHandler.resetAll();
    this.feedbackHandler = newHandler;
  }
}

document.addEventListener("DOMContentLoaded", () => {
  window.gridController = new GridController();
});

document.addEventListener("cellpress", (e) => {
  console.log(`Cell ${e.detail.index} pressed -> Key: ${e.detail.key}`);
});

document.addEventListener("cellrelease", (e) => {
  if (e.detail.reason === "slid_away") {
    console.log(`Cell ${e.detail.index} released (slid away) -> Key: ${e.detail.key}`);
  } else if (e.detail.reason === "moved_outside") {
    console.log(`Cell ${e.detail.index} released (moved outside grid) -> Key: ${e.detail.key}`);
  } else {
    console.log(`Cell ${e.detail.index} released -> Key: ${e.detail.key}`);
  }
});

document.addEventListener("contextmenu", (e) => {
  if (e.target.classList.contains("grid-cell")) {
    e.preventDefault();
  }
});
