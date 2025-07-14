// WebSocket handler for sending input events to Backflow
class WebSocketHandler {
  // todo: don't hardcode the ip
  constructor(url = `ws://${window.location.host}/ws`) {
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
        console.log("🟢 Connected to Backflow WebSocket");
        this.isConnected = true;
        this.reconnectAttempts = 0;
      };

      this.ws.onmessage = (event) => {
        console.log("📥 Received from Backflow:", event.data);
        try {
          const data = JSON.parse(event.data);
          if (data.events && Array.isArray(data.events)) {
            this.handleFeedbackPacket(data);
          }
        } catch (e) {
          console.warn(
            "Failed to parse WebSocket message as feedback packet:",
            e,
          );
        }
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
      console.log(
        `🔄 Attempting to reconnect... (${this.reconnectAttempts}/${this.maxReconnectAttempts})`,
      );
      setTimeout(() => {
        this.connect();
      }, this.reconnectDelay * this.reconnectAttempts);
    } else {
      console.error(
        "Max reconnection attempts reached. Please refresh the page.",
      );
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
      events: events,
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
    const events = [
      {
        Keyboard: {
          [eventType]: {
            key: key,
          },
        },
      },
    ];

    // Add retry logic for failed sends
    const sendWithRetry = (attempt = 1) => {
      const success = this.sendInputEvent(deviceId, events);
      if (!success && attempt < 3) {
        console.warn(
          `🔄 Keyboard event send failed (attempt ${attempt}), retrying...`,
        );
        setTimeout(() => sendWithRetry(attempt + 1), 10 * attempt); // exponential backoff
      } else if (!success) {
        console.error(
          `❌ Keyboard event send failed after 3 attempts: ${key} ${eventType}`,
        );
      }
    };

    sendWithRetry();
  }

  sendPointerEvent(eventType, data, deviceId = "chunitroller-webapp") {
    const events = [
      {
        Pointer: {
          [eventType]: data,
        },
      },
    ];

    return this.sendInputEvent(deviceId, events);
  }

  handleFeedbackPacket(packet) {
    console.log("🎨 Processing feedback packet:", packet);

    // Batch DOM updates for better performance
    const ledUpdates = [];

    // Collect all LED updates first
    for (const event of packet.events) {
      if (event.Led && event.Led.Set) {
        ledUpdates.push(event.Led.Set);
      }
    }

    // Apply all LED updates in a single batch to minimize DOM reflows
    if (ledUpdates.length > 0) {
      // Use requestAnimationFrame to batch all DOM updates together
      requestAnimationFrame(() => {
        for (const ledEvent of ledUpdates) {
          this.handleLedEvent(ledEvent);
        }
      });
    }
  }

  handleLedEvent(ledEvent) {
    const { led_id, on, brightness, rgb } = ledEvent;
    console.log(
      `💡 LED ${led_id}: on=${on}, brightness=${brightness}, rgb=${rgb}`,
    );

    // Find the corresponding grid cell
    const cell = this.getCellByLedId(led_id);
    if (cell) {
      this.applyLedToCell(cell, on, brightness, rgb);
    }
  }

  getCellByLedId(ledId) {
    // Map LED IDs to grid cells with buttons first, air sensors last
    // LED IDs 0-15: Slider buttons (16 keys: q,w,e,r,t,y,u,i,o,p,a,s,d,f,g,h)
    // LED IDs 16-21: Air sensors (6 sensors: 1,2,3,4,5,6)

    const airSensors = document.querySelectorAll(
      '[data-cell-section="air-sensor"] .grid-cell',
    );
    const sliderButtons = document.querySelectorAll(
      '[data-cell-section="slider"] .grid-cell',
    );

    if (ledId >= 0 && ledId < 16) {
      // Slider buttons (LED IDs 0-15)
      return sliderButtons[ledId] || null;
    } else if (ledId >= 16 && ledId < 22) {
      // Air sensors (LED IDs 16-21)
      const airIndex = ledId - 16;
      return airSensors[airIndex] || null;
    }

    return null;
  }

  applyLedToCell(cell, on, brightness, rgb) {
    if (!on) {
      // Turn off LED - remove RGB styling but keep any existing active state
      cell.style.removeProperty("background-color");
      cell.style.removeProperty("box-shadow");
      cell.classList.remove("rgb-active");
      cell.removeAttribute("data-rgb-color");
      return;
    }

    let color = "white";
    let r = 255,
      g = 255,
      b = 255;

    if (rgb && Array.isArray(rgb) && rgb.length >= 3) {
      [r, g, b] = rgb;
      color = `rgb(${r}, ${g}, ${b})`;
    }

    // Apply brightness if specified
    if (brightness !== null && brightness !== undefined) {
      const alpha = brightness / 255;
      color = `rgba(${r}, ${g}, ${b}, ${alpha})`;
    }

    // Store the RGB color for reference and create inset glow effect
    cell.setAttribute("data-rgb-color", color);
    cell.style.backgroundColor = color;

    // Use inset box-shadow to prevent layout shifts, with the RGB color for glow
    const glowColor = `rgba(${r}, ${g}, ${b}, 0.6)`;
    cell.style.boxShadow = `inset 0 0 20px ${glowColor}`;
    cell.classList.add("rgb-active");
  }

  close() {
    if (this.ws) {
      this.ws.close();
      this.isConnected = false;
    }
  }
}

class CellFeedbackHandler {
  constructor() {
    this.activeCells = new Set();
  }

  activateCell(cell) {
    if (!this.activeCells.has(cell)) {
      console.log("🟢 Activating cell:", cell.getAttribute("data-key"));
      cell.classList.add("active");
      this.activeCells.add(cell);
    }
  }

  deactivateCell(cell) {
    if (this.activeCells.has(cell)) {
      console.log("🔴 Deactivating cell:", cell.getAttribute("data-key"));
      cell.classList.remove("active");
      this.activeCells.delete(cell);
    }
  }

  resetAll() {
    this.activeCells.forEach((cell) => {
      cell.classList.remove("active");
    });
    this.activeCells.clear();
  }

  // Get count of currently active cells
  getActiveCount() {
    return this.activeCells.size;
  }

  // Check if a specific cell is active
  isCellActive(cell) {
    return this.activeCells.has(cell);
  }
}
class GridController {
  constructor() {
    this.feedbackHandler = new CellFeedbackHandler();
    this.webSocketHandler = new WebSocketHandler();
    this.cells = [];
    this.activeTouches = new Map(); // touchId -> {cell, index, startTime}
    this.cellTouchCounts = new Map(); // cellIndex -> number of active touches (for keyboard events)
    this.cellVisualTouches = new Map(); // cellIndex -> Set of touch IDs (for visual feedback)
    this.pressedKeys = new Map(); // key -> {startTime, cellIndex} - tracks which keys are currently pressed
    this.touchCounter = null; // Will be set in init
    this.stuckKeyTimeout = 3000; // 3 seconds timeout for stuck keys (reduced from 5s)
    this.cleanupInterval = null; // For periodic cleanup
    this.multitouchDeadzone = 100; // ms - debounce period for rapid multitouch events
    this.lastEventTimestamp = new Map(); // key -> timestamp to prevent rapid duplicate events
    this.pendingKeyEvents = new Map(); // key -> {type: 'press'|'release', timestamp} for debouncing
    this.keyEventDebounceMs = 16; // ~60fps debouncing for key events
    this.cellOverlapEnabled = true; // Enable cell overlap by default
    this.edgeOverlapThreshold = 15; // Pixels from edge to trigger overlap
    this.overlapActivations = new Map(); // touchId -> array of overlapped cell indices

    this.init();
  }

  init() {
    this.cells = document.querySelectorAll(".grid-cell");
    this.touchCounter = document.getElementById("touchCounter");
    this.cells.forEach((cell, index) => {
      cell.setAttribute("data-cell-index", index);
      this.cellTouchCounts.set(index, 0);
      this.cellVisualTouches.set(index, new Set());
    });
    this.setupGlobalTouchHandlers();
    this.updateTouchCounter();
    // this.startStuckKeyCleanup();

    console.log(`Grid controller initialized with ${this.cells.length} cells`);
  }

  // Debounce keyboard events
  sendDebouncedKeyboardEvent(key, pressed, deviceName) {
    const now = Date.now();
    const eventType = pressed ? "press" : "release";
    const lastEventTime = this.lastEventTimestamp.get(key) || 0;

    if (now - lastEventTime < this.keyEventDebounceMs) {
      // event queue
      this.pendingKeyEvents.set(key, {
        type: eventType,
        timestamp: now,
        deviceName: deviceName,
        pressed: pressed,
      });

      // Clear any existing timeout for this key
      const timeoutKey = `timeout_${key}`;
      if (this[timeoutKey]) {
        clearTimeout(this[timeoutKey]);
      }

      // Set a new timeout to send the queued event
      this[timeoutKey] = setTimeout(() => {
        const pendingEvent = this.pendingKeyEvents.get(key);
        if (pendingEvent) {
          this.webSocketHandler.sendKeyboardEvent(
            pendingEvent.pressed ? key : key,
            pendingEvent.pressed,
            pendingEvent.deviceName,
          );
          this.lastEventTimestamp.set(key, pendingEvent.timestamp);
          this.pendingKeyEvents.delete(key);
        }
        delete this[timeoutKey];
      }, this.keyEventDebounceMs);

      return;
    }

    // Send immediately if enough time has passed
    this.webSocketHandler.sendKeyboardEvent(key, pressed, deviceName);
    this.lastEventTimestamp.set(key, now);
  }

  startStuckKeyCleanup() {
    // this was causing legitimate key holds to be released
    // this.cleanupInterval = setInterval(() => {
    //   this.cleanupStuckKeys();
    //   this.cleanupOrphanedTouches(); // Add orphaned touch cleanup
    //   this.validateAndCleanupState(); // Add state validation
    // }, 500); // Check every 500ms instead of 1000ms for better responsiveness
  }

  // Clean up touches that may have been orphaned during rapid multitouch
  cleanupOrphanedTouches() {
    const now = Date.now();
    const orphanTimeout = 2000; // 2 seconds

    // Check for touches that have been active too long without movement
    const orphanedTouches = [];
    this.activeTouches.forEach((touchData, touchId) => {
      if (now - touchData.startTime > orphanTimeout) {
        orphanedTouches.push(touchId);
      }
    });

    // Clean up orphaned touches
    orphanedTouches.forEach((touchId) => {
      console.warn(`🧹 Cleaning up orphaned touch: ${touchId}`);
      this.handleTouchEnd({ identifier: touchId }, "orphaned_cleanup");
    });
  }

  // Validate state consistency and clean up any inconsistencies
  validateAndCleanupState() {
    const now = Date.now();
    let fixedIssues = 0;

    // Check for pressed keys that don't have corresponding touch counts
    this.pressedKeys.forEach((pressInfo, key) => {
      const cellIndex = pressInfo.cellIndex;
      const touchCount = this.cellTouchCounts.get(cellIndex) || 0;

      // If a key is pressed but no touches are tracked for that cell, it's stuck
      if (touchCount === 0) {
        console.warn(
          `🔧 Found stuck key ${key} with no active touches, force releasing`,
        );
        this.forceReleaseKey(key, cellIndex, "state_validation_cleanup");
        fixedIssues++;
      }
    });

    // Check for touch counts that don't have corresponding active touches
    this.cellTouchCounts.forEach((count, cellIndex) => {
      if (count > 0) {
        // Count how many active touches actually reference this cell
        let actualTouches = 0;
        this.activeTouches.forEach((touchData) => {
          if (touchData.index === cellIndex) {
            actualTouches++;
          }
        });

        if (actualTouches !== count) {
          console.warn(
            `🔧 Cell ${cellIndex} has touch count ${count} but only ${actualTouches} active touches, correcting`,
          );
          this.cellTouchCounts.set(cellIndex, actualTouches);

          // If no actual touches but we have a key pressed for this cell, release it
          if (actualTouches === 0) {
            const cell = this.cells[cellIndex];
            if (cell) {
              const key = this.mapCellToKey(cell);
              if (this.pressedKeys.has(key)) {
                this.forceReleaseKey(key, cellIndex, "touch_count_correction");
                fixedIssues++;
              }
            }
          }
        }
      }
    });

    if (fixedIssues > 0) {
      console.log(`🔧 State validation fixed ${fixedIssues} issues`);
    }

    return fixedIssues;
  }

  // Clean up keys that have been pressed for too long - DISABLED
  cleanupStuckKeys() {
    // This was causing legitimate long key holds to be released
    // commenting out for now - rely on touch end events instead
    /*
    const now = Date.now();
    const stuckKeys = [];

    this.pressedKeys.forEach((pressInfo, key) => {
      if (now - pressInfo.startTime > this.stuckKeyTimeout) {
        stuckKeys.push({key, pressInfo});
      }
    });

    stuckKeys.forEach(({key, pressInfo}) => {
      console.warn(`🔥 STUCK KEY DETECTED: ${key} (pressed for ${now - pressInfo.startTime}ms), force releasing`);

      // Force release the key
      this.forceReleaseKey(key, pressInfo.cellIndex, "stuck_key_cleanup");
    });
    */
  }

  // Force release a key and clean up all associated state
  forceReleaseKey(key, cellIndex, reason = "force_release") {
    // Send keyboard release event
    const deviceName = this.getDeviceNameForCell(this.cells[cellIndex]);
    this.sendDebouncedKeyboardEvent(key, false, deviceName);

    // Remove from pressed keys tracking
    this.pressedKeys.delete(key);

    // Reset touch counts for this cell
    this.cellTouchCounts.set(cellIndex, 0);

    // Clear visual touches for this cell
    this.cellVisualTouches.get(cellIndex)?.clear();

    // Clear visual feedback
    const cell = this.cells[cellIndex];
    if (cell) {
      this.feedbackHandler.deactivateCell(cell);

      // Dispatch release event
      cell.dispatchEvent(
        new CustomEvent("cellrelease", {
          detail: {
            index: cellIndex,
            cell: cell,
            key: key,
            reason: reason,
          },
          bubbles: true,
        }),
      );
    }

    console.log(`🔥 Force released key: ${key} (reason: ${reason})`);
  }

  setupGlobalTouchHandlers() {
    // mouse support (dont need, mostly only for test)
    document.addEventListener("mousedown", (e) => {
      if (e.target.classList.contains("grid-cell")) {
        console.log(
          "🖱️ Mouse down on cell:",
          e.target.getAttribute("data-key"),
        );
        e.preventDefault();
        const cellIndex = parseInt(e.target.getAttribute("data-cell-index"));
        if (cellIndex !== null && !isNaN(cellIndex)) {
          this.activateCellWithOverlap(e.target, e.clientX, e.clientY, "mouse");

          this.updateTouchCounter();
        }
      }
    });

    document.addEventListener("mouseup", (e) => {
      if (this.activeTouches.has("mouse")) {
        console.log("🖱️ Mouse up");
        this.handleTouchEnd({ identifier: "mouse" }, "mouse_up");
        this.updateTouchCounter();
      }
    });

    document.addEventListener(
      "touchstart",
      (e) => {
        console.log(
          "👆 Touch start detected, touches:",
          e.changedTouches.length,
        );
        e.preventDefault();
        Array.from(e.changedTouches).forEach((touch) => {
          // Skip if we're already tracking this touch (shouldn't happen but safety check)
          if (this.activeTouches.has(touch.identifier)) {
            console.warn(
              `👆 Duplicate touch start for ID ${touch.identifier}, ignoring`,
            );
            return;
          }

          const elementUnderTouch = document.elementFromPoint(
            touch.clientX,
            touch.clientY,
          );

          console.log(
            "👆 Element under touch:",
            elementUnderTouch?.tagName,
            elementUnderTouch?.getAttribute("data-key"),
          );

          if (
            elementUnderTouch &&
            elementUnderTouch.classList.contains("grid-cell")
          ) {
            const cellIndex = parseInt(
              elementUnderTouch.getAttribute("data-cell-index"),
            );
            console.log("👆 Touch on grid cell, index:", cellIndex);
            if (cellIndex !== null && !isNaN(cellIndex)) {
              // Use new overlap activation method
              this.activateCellWithOverlap(
                elementUnderTouch,
                touch.clientX,
                touch.clientY,
                touch.identifier,
              );

              this.updateTouchCounter();
            }
          }
        });
      },
      { passive: false },
    );

    document.addEventListener(
      "touchmove",
      (e) => {
        if (this.activeTouches.size > 0) {
          e.preventDefault();
          this.handleTouchMove(e);
        }
      },
      { passive: false },
    );

    document.addEventListener(
      "touchend",
      (e) => {
        Array.from(e.changedTouches).forEach((touch) => {
          this.handleTouchEnd(touch);
        });
        this.updateTouchCounter();
      },
      { passive: false },
    );

    document.addEventListener(
      "touchcancel",
      (e) => {
        Array.from(e.changedTouches).forEach((touch) => {
          this.handleTouchEnd(touch, "touch_cancel");
        });
        this.updateTouchCounter();
      },
      { passive: false },
    );
  }

  handleTouchEnd(touch, reason = "touch_end") {
    const touchData = this.activeTouches.get(touch.identifier);
    if (touchData) {
      const cellIndex = touchData.index;

      console.log("🔴 Touch end - cellIndex:", cellIndex, "reason:", reason);

      // Remove this touch from visual tracking
      const visualTouches = this.cellVisualTouches.get(cellIndex);
      if (visualTouches) {
        visualTouches.delete(touch.identifier);

        // Only deactivate visual feedback if no touches remain on this cell
        if (visualTouches.size === 0) {
          console.log(
            "🔴 Last visual touch on cell, deactivating visual feedback",
          );
          this.feedbackHandler.deactivateCell(touchData.cell);
        }
      }

      // Decrement touch count for keyboard events
      const currentCount = this.cellTouchCounts.get(cellIndex) || 1;
      const newCount = Math.max(0, currentCount - 1);
      this.cellTouchCounts.set(cellIndex, newCount);

      console.log("🔴 Touch counts - was:", currentCount, "now:", newCount);

      // Only send keyboard release if this was the last touch on this cell
      if (newCount === 0) {
        console.log("🔴 Last keyboard touch on cell, sending keyboard release");

        const key = this.mapCellToKey(touchData.cell);
        const deviceName = this.getDeviceNameForCell(touchData.cell);

        if (this.pressedKeys.has(key)) {
          this.pressedKeys.delete(key);

          this.sendDebouncedKeyboardEvent(key, false, deviceName);

          touchData.cell.dispatchEvent(
            new CustomEvent("cellrelease", {
              detail: {
                index: touchData.index,
                cell: touchData.cell,
                key: key,
                reason: reason,
              },
              bubbles: true,
            }),
          );
        } else {
          console.warn(
            `🚨 Attempted to release key ${key} that wasn't tracked as pressed`,
          );
        }
      } else {
        console.log(
          "🔴 Still",
          newCount,
          "keyboard touches on cell, keeping keyboard pressed",
        );
      }

      // Clean up any overlap activations for this touch
      if (this.overlapActivations.has(touch.identifier)) {
        const overlapCellIndices = this.overlapActivations.get(
          touch.identifier,
        );
        console.log(
          `🔴 Cleaning up ${overlapCellIndices.length} overlap activations for touch ${touch.identifier}`,
        );

        overlapCellIndices.forEach((cellIndex) => {
          const overlapTouchId = `${touch.identifier}_overlap_${cellIndex}`;
          this.deactivateCell(cellIndex, overlapTouchId);
        });

        this.overlapActivations.delete(touch.identifier);
      }

      this.activeTouches.delete(touch.identifier);
    } else {
      console.log(
        "🔴 Touch end but no touch data found for:",
        touch.identifier,
      );

      this.validateAndCleanupState();
    }
  }

  handleCellPress(cell, index) {
    console.log(
      "🎯 handleCellPress called for cell:",
      cell.getAttribute("data-key"),
      "index:",
      index,
    );

    // This method is now mainly for legacy compatibility
    // The actual logic is handled directly in touch/mouse event handlers
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

  // Detect if touch is near edge of cell and find adjacent cells
  detectEdgeOverlap(cell, touchX, touchY) {
    if (!this.cellOverlapEnabled) {
      return [];
    }

    const rect = cell.getBoundingClientRect();
    const relativeX = touchX - rect.left;
    const relativeY = touchY - rect.top;

    const threshold = this.edgeOverlapThreshold;
    const adjacentCells = [];

    // near edges? overlap!
    const nearLeftEdge = relativeX <= threshold;
    const nearRightEdge = relativeX >= rect.width - threshold;
    const nearTopEdge = relativeY <= threshold;
    const nearBottomEdge = relativeY >= rect.height - threshold;

    const currentIndex = parseInt(cell.getAttribute("data-cell-index"));
    const section = cell.closest("[data-cell-section]");
    const sectionName = section
      ? section.getAttribute("data-cell-section")
      : "unknown";
    const isHorizontalLayout =
      section && section.classList.contains("horizontal");

    console.log(
      `🔀 Overlap check: cell ${currentIndex} in ${sectionName} (${isHorizontalLayout ? "vertical" : "horizontal"} layout)`,
    );
    console.log(
      `🔀 Touch at (${relativeX.toFixed(1)}, ${relativeY.toFixed(1)}) in ${rect.width.toFixed(1)}x${rect.height.toFixed(1)} cell, threshold: ${threshold}px`,
    );
    console.log(
      `🔀 Edges: left=${nearLeftEdge}, right=${nearRightEdge}, top=${nearTopEdge}, bottom=${nearBottomEdge}`,
    );

    if (!nearLeftEdge && !nearRightEdge && !nearTopEdge && !nearBottomEdge) {
      console.log(`🔀 Not near any edge, no overlap`);
      return []; // this is all just debug prints i think we can get rid of those? idk
    }

    // Get cells in the same section only
    const sectionCells = section
      ? Array.from(section.querySelectorAll(".grid-cell"))
      : [];
    const sectionIndices = sectionCells.map((c) =>
      parseInt(c.getAttribute("data-cell-index")),
    );
    const currentSectionPosition = sectionIndices.indexOf(currentIndex);

    if (currentSectionPosition === -1) return [];

    if (isHorizontalLayout) {
      if (nearTopEdge && currentSectionPosition > 0) {
        const adjacentIndex = sectionIndices[currentSectionPosition - 1];
        const adjacentCell = this.getCell(adjacentIndex);
        if (adjacentCell) adjacentCells.push(adjacentCell);
      }

      if (
        nearBottomEdge &&
        currentSectionPosition < sectionIndices.length - 1
      ) {
        const adjacentIndex = sectionIndices[currentSectionPosition + 1];
        const adjacentCell = this.getCell(adjacentIndex);
        if (adjacentCell) adjacentCells.push(adjacentCell);
      }
    } else {
      if (nearLeftEdge && currentSectionPosition > 0) {
        const adjacentIndex = sectionIndices[currentSectionPosition - 1];
        const adjacentCell = this.getCell(adjacentIndex);
        if (adjacentCell) adjacentCells.push(adjacentCell);
      }

      if (nearRightEdge && currentSectionPosition < sectionIndices.length - 1) {
        const adjacentIndex = sectionIndices[currentSectionPosition + 1];
        const adjacentCell = this.getCell(adjacentIndex);
        if (adjacentCell) adjacentCells.push(adjacentCell);
      }
    }

    return adjacentCells;
  }

  getLayoutInfo() {
    if (!this.cells || this.cells.length === 0) {
      return { type: "unknown", cellsPerRow: 1 };
    }

    const firstCell = this.cells[0];
    const section = firstCell.closest("[data-cell-section]");
    const isHorizontalLayout =
      section && section.classList.contains("horizontal");

    if (isHorizontalLayout) {
      return { type: "vertical", cellsPerRow: 1 };
    } else {
      return { type: "horizontal", cellsPerRow: this.cells.length };
    }
  }

  activateCellWithOverlap(primaryCell, touchX, touchY, touchId) {
    const primaryIndex = parseInt(primaryCell.getAttribute("data-cell-index"));

    this.activeTouches.set(touchId, {
      cell: primaryCell,
      index: primaryIndex,
      startTime: Date.now(),
    });

    this.activateCell(primaryCell, primaryIndex, touchId, false);

    const adjacentCells = this.detectEdgeOverlap(primaryCell, touchX, touchY);

    if (adjacentCells.length > 0) {
      const section = primaryCell.closest("[data-cell-section]");
      const sectionName = section
        ? section.getAttribute("data-cell-section")
        : "unknown";
      const isHorizontalLayout =
        section && section.classList.contains("horizontal");
      const layoutType = isHorizontalLayout ? "vertical" : "horizontal";
      console.log(
        `🔀 Edge touch detected in ${sectionName} (${layoutType} layout), activating ${adjacentCells.length} adjacent cells`,
      );

      this.overlapActivations.set(
        touchId,
        adjacentCells.map((cell) =>
          parseInt(cell.getAttribute("data-cell-index")),
        ),
      );

      adjacentCells.forEach((cell) => {
        const adjIndex = parseInt(cell.getAttribute("data-cell-index"));
        console.log(`🔀 Activating adjacent cell ${adjIndex} due to overlap`);
        this.activateCell(
          cell,
          adjIndex,
          `${touchId}_overlap_${adjIndex}`,
          true,
        );
      });
    } else {
      console.log(`🔀 No edge overlap detected for cell ${primaryIndex}`);
    }
  }

  activateCell(cell, cellIndex, touchId, isOverlap = false) {
    const currentCount = this.cellTouchCounts.get(cellIndex) || 0;
    this.cellTouchCounts.set(cellIndex, currentCount + 1);

    const visualTouches = this.cellVisualTouches.get(cellIndex);
    if (visualTouches) {
      visualTouches.add(touchId);
    }

    this.feedbackHandler.activateCell(cell);

    // Only send keyboard press event if this is the first touch on this cell
    if (currentCount === 0) {
      const touchType = isOverlap ? "overlap" : "direct";
      console.log(
        `👆 First ${touchType} touch on cell ${cellIndex}, sending keyboard event`,
      );

      const key = this.mapCellToKey(cell);
      const deviceName = this.getDeviceNameForCell(cell);

      if (!this.pressedKeys.has(key)) {
        // Track this key as pressed
        this.pressedKeys.set(key, {
          startTime: Date.now(),
          cellIndex: cellIndex,
        });

        this.sendDebouncedKeyboardEvent(key, true, deviceName);

        cell.dispatchEvent(
          new CustomEvent("cellpress", {
            detail: {
              index: cellIndex,
              cell: cell,
              key: key,
              isOverlap: isOverlap,
            },
            bubbles: true,
          }),
        );
      } else {
        console.warn(
          `👆 Key ${key} already pressed, not sending duplicate press event`,
        );
      }
    }
  }

  deactivateCell(cellIndex, touchId) {
    const cell = this.getCell(cellIndex);
    if (!cell) return;

    const visualTouches = this.cellVisualTouches.get(cellIndex);
    if (visualTouches) {
      visualTouches.delete(touchId);

      if (visualTouches.size === 0) {
        console.log(
          `🔴 Last visual touch on overlap cell ${cellIndex}, deactivating visual feedback`,
        );
        this.feedbackHandler.deactivateCell(cell);
      }
    }
    const currentCount = this.cellTouchCounts.get(cellIndex) || 1;
    const newCount = Math.max(0, currentCount - 1);
    this.cellTouchCounts.set(cellIndex, newCount);

    if (newCount === 0) {
      console.log(
        `🔴 Last keyboard touch on overlap cell ${cellIndex}, sending keyboard release`,
      );

      const key = this.mapCellToKey(cell);
      const deviceName = this.getDeviceNameForCell(cell);

      if (this.pressedKeys.has(key)) {
        this.pressedKeys.delete(key);

        this.sendDebouncedKeyboardEvent(key, false, deviceName);

        cell.dispatchEvent(
          new CustomEvent("cellrelease", {
            detail: {
              index: cellIndex,
              cell: cell,
              key: key,
              reason: "overlap_cleanup",
            },
            bubbles: true,
          }),
        );
      }
    }
  }

  // Map cell to keyboard key using data-key attribute
  mapCellToKey(cellOrIndex) {
    let cell;
    if (typeof cellOrIndex === "number") {
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

    const dataKey = cell.getAttribute("data-key");
    if (!dataKey) {
      console.warn("Cell has no data-key attribute:", cell);
      return "KEY_UNKNOWN";
    }

    // Convert the data-key to final key code format
    return dataKey;
  }

  // Update the touch counter display
  updateTouchCounter() {
    if (this.touchCounter) {
      const activeCount = this.activeTouches.size;
      const activeCellCount = this.feedbackHandler.getActiveCount();
      const pressedKeyCount = this.pressedKeys.size;
      this.touchCounter.textContent = `Touches: ${activeCount} | Cells: ${activeCellCount} | Keys: ${pressedKeyCount}`;
    }
  }

  // Get debug information about current touch state
  getTouchDebugInfo() {
    const now = Date.now();
    return {
      activeTouches: this.activeTouches.size,
      pressedKeys: this.pressedKeys.size,
      pressedKeyDetails: Array.from(this.pressedKeys.entries()).map(
        ([key, info]) => ({
          key: key,
          duration: now - info.startTime,
          cellIndex: info.cellIndex,
        }),
      ),
      touchData: Array.from(this.activeTouches.entries()).map(([id, data]) => ({
        touchId: id,
        cellIndex: data.index,
        startTime: data.startTime,
        duration: now - data.startTime,
      })),
      cellTouchCounts: Array.from(this.cellTouchCounts.entries())
        .filter(([_, count]) => count > 0)
        .map(([index, count]) => ({ cellIndex: index, touchCount: count })),
    };
  }

  // Reset all touch state (useful for debugging)
  resetTouchState() {
    // Force release all currently pressed keys
    this.pressedKeys.forEach((pressInfo, key) => {
      this.forceReleaseKey(key, pressInfo.cellIndex, "manual_reset");
    });

    this.activeTouches.clear();
    this.pressedKeys.clear();
    this.cellTouchCounts.forEach((_, index) => {
      this.cellTouchCounts.set(index, 0);
    });
    this.cellVisualTouches.forEach((touchSet, index) => {
      touchSet.clear();
    });
    this.feedbackHandler.resetAll();
    this.updateTouchCounter();
    console.log("Touch state reset - all keys force released");
  }

  // Get device name based on cell's section
  getDeviceNameForCell(cell) {
    // Find the parent container with data-cell-section
    let container = cell.closest("[data-cell-section]");
    if (container) {
      const section = container.getAttribute("data-cell-section");
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
        const newCellIndex = parseInt(
          elementUnderTouch.getAttribute("data-cell-index"),
        );
        if (
          newCellIndex !== null &&
          !isNaN(newCellIndex) &&
          newCellIndex !== touchData.index
        ) {
          // Moving to a different cell
          const oldCellIndex = touchData.index;

          console.log(
            "🔄 Touch sliding from cell",
            oldCellIndex,
            "to cell",
            newCellIndex,
          );

          if (this.overlapActivations.has(touch.identifier)) {
            const oldOverlapCellIndices = this.overlapActivations.get(
              touch.identifier,
            );
            oldOverlapCellIndices.forEach((cellIndex) => {
              const overlapTouchId = `${touch.identifier}_overlap_${cellIndex}`;
              this.deactivateCell(cellIndex, overlapTouchId);
            });
            this.overlapActivations.delete(touch.identifier);
          }

          this.deactivateCell(oldCellIndex, touch.identifier);

          this.activeTouches.set(touch.identifier, {
            cell: elementUnderTouch,
            index: newCellIndex,
            startTime: touchData.startTime,
          });

          this.activateCellWithOverlap(
            elementUnderTouch,
            touch.clientX,
            touch.clientY,
            touch.identifier,
          );
        }
      } else {
        this.handleTouchEnd(touch, "moved_outside");
      }
    });
  }

  getCell(index) {
    return this.cells[index] || null;
  }

  getCellCount() {
    return this.cells.length;
  }

  setFeedbackHandler(handler) {
    this.feedbackHandler = handler;
  }

  setCellOverlapEnabled(enabled) {
    this.cellOverlapEnabled = enabled;
    console.log(`🔀 Cell overlap ${enabled ? "enabled" : "disabled"}`);
  }

  setEdgeOverlapThreshold(pixels) {
    this.edgeOverlapThreshold = Math.max(1, pixels);
    console.log(
      `🔀 Edge overlap threshold set to ${this.edgeOverlapThreshold}px`,
    );
  }

  // Cleanup method to stop intervals and release resources
  destroy() {
    // if (this.cleanupInterval) {
    //   clearInterval(this.cleanupInterval);
    //   this.cleanupInterval = null;
    // }

    // Force release all pressed keys
    this.pressedKeys.forEach((pressInfo, key) => {
      this.forceReleaseKey(key, pressInfo.cellIndex, "destroy");
    });

    console.log("GridController destroyed and cleaned up");
  }

  // Manual method to release all stuck keys (for debugging)
  releaseAllStuckKeys() {
    const now = Date.now();
    let releasedCount = 0;

    this.pressedKeys.forEach((pressInfo, key) => {
      console.log(
        `🔥 Force releasing potentially stuck key: ${key} (pressed for ${now - pressInfo.startTime}ms)`,
      );
      this.forceReleaseKey(
        key,
        pressInfo.cellIndex,
        "manual_stuck_key_release",
      );
      releasedCount++;
    });

    console.log(`🔥 Released ${releasedCount} potentially stuck keys`);
    return releasedCount;
  }
}

document.addEventListener("DOMContentLoaded", () => {
  window.gridController = new GridController();

  window.debugTouch = () => {
    console.log("Touch Debug Info:", window.gridController.getTouchDebugInfo());
  };

  window.resetTouch = () => {
    window.gridController.resetTouchState();
  };

  window.releaseStuckKeys = () => {
    return window.gridController.releaseAllStuckKeys();
  };

  window.validateState = () => {
    return window.gridController.validateAndCleanupState();
  };

  window.getDetailedState = () => {
    const grid = window.gridController;
    return {
      activeTouches: Array.from(grid.activeTouches.entries()),
      cellTouchCounts: Array.from(grid.cellTouchCounts.entries()),
      pressedKeys: Array.from(grid.pressedKeys.entries()),
      pendingKeyEvents: Array.from(grid.pendingKeyEvents.entries()),
    };
  };

  window.testCell = (key) => {
    const cell = document.querySelector(`[data-key="${key}"]`);
    if (cell) {
      console.log("🧪 Testing cell activation for key:", key);
      cell.classList.add("active");
      setTimeout(() => {
        cell.classList.remove("active");
        console.log("🧪 Test complete");
      }, 1000);
    } else {
      console.log("🧪 Cell not found for key:", key);
    }
  };

  window.setCellOverlap = (enabled) => {
    window.gridController.setCellOverlapEnabled(enabled);
  };

  window.setOverlapThreshold = (pixels) => {
    window.gridController.setEdgeOverlapThreshold(pixels);
  };

  window.getOverlapSettings = () => {
    const grid = window.gridController;
    return {
      enabled: grid.cellOverlapEnabled,
      threshold: grid.edgeOverlapThreshold,
      activeOverlaps: Array.from(grid.overlapActivations.entries()),
    };
  };

  console.log("backflow loaded");
  console.log(
    "🔧 debug commands: debugTouch(), resetTouch(), releaseStuckKeys(), validateState(), getDetailedState(), testCell('q')",
  );
  console.log(
    "🔀 overlap commands: setCellOverlap(true/false), setOverlapThreshold(pixels), getOverlapSettings()",
  );
});

document.addEventListener("cellpress", (e) => {
  const touchInfo = window.gridController?.getTouchDebugInfo();
  console.log(
    `🟢 Cell ${e.detail.index} pressed -> Key: ${e.detail.key} (Active touches: ${touchInfo?.activeTouches || 0})`,
  );
});

document.addEventListener("cellrelease", (e) => {
  const touchInfo = window.gridController?.getTouchDebugInfo();
  let reasonText = "";
  if (e.detail.reason === "slid_away") {
    reasonText = " (slid away)";
  } else if (e.detail.reason === "moved_outside") {
    reasonText = " (moved outside grid)";
  } else if (e.detail.reason === "touch_cancel") {
    reasonText = " (touch cancelled)";
  }
  console.log(
    `🔴 Cell ${e.detail.index} released${reasonText} -> Key: ${e.detail.key} (Active touches: ${touchInfo?.activeTouches || 0})`,
  );
});

document.addEventListener("contextmenu", (e) => {
  if (e.target.classList.contains("grid-cell")) {
    e.preventDefault();
  }
});
