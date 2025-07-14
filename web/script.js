const throttle = (func, wait) => {
  var ready = true;
  var args = null;
  return function throttled() {
    var context = this;
    if (ready) {
      ready = false;
      setTimeout(function () {
        ready = true;
        if (args) {
          throttled.apply(context);
        }
      }, wait);
      if (args) {
        func.apply(this, args);
        args = null;
      } else {
        func.apply(this, arguments);
      }
    } else {
      args = arguments;
    }
  };
};

class WebSocketHandler {
  constructor(url = `ws://${window.location.host}/ws`) {
    this.url = url;
    this.ws = null;
    this.isConnected = false;
    this.reconnectAttempts = 0;
    this.maxReconnectAttempts = 5;
    this.reconnectDelay = 1000;
    this.connect();
  }

  connect() {
    try {
      this.ws = new WebSocket(this.url);

      this.ws.onopen = () => {
        console.log("connected to backflow websocket");
        this.isConnected = true;
        this.reconnectAttempts = 0;
      };

      this.ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          if (data.events && Array.isArray(data.events)) {
            this.handleFeedbackPacket(data);
          }
        } catch (e) {
          console.warn("Failed to parse WebSocket message:", e);
        }
      };

      this.ws.onclose = () => {
        console.log("websocket connection closed");
        this.isConnected = false;
        this.attemptReconnect();
      };

      this.ws.onerror = (error) => {
        console.error("websocket error:", error);
        this.isConnected = false;
      };
    } catch (error) {
      console.error("failed to create websocket connection:", error);
      this.attemptReconnect();
    }
  }

  attemptReconnect() {
    if (this.reconnectAttempts < this.maxReconnectAttempts) {
      this.reconnectAttempts++;
      console.log(
        `attempting to reconnect... (${this.reconnectAttempts}/${this.maxReconnectAttempts})`,
      );
      setTimeout(() => {
        this.connect();
      }, this.reconnectDelay * this.reconnectAttempts);
    } else {
      console.error("max reconnection attempts reached");
    }
  }

  sendKeyEvents(keyStates) {
    if (!this.isConnected || !this.ws) {
      return false;
    }

    for (let i = 0; i < keyStates.length; i++) {
      if (keyStates[i] !== this.lastKeyStates[i]) {
        const cell = document.querySelector(`[data-cell-index="${i}"]`);
        if (cell) {
          const key = cell.getAttribute("data-key");
          const deviceId = this.getDeviceNameForCell(cell);
          const pressed = keyStates[i] === 1;

          const event = {
            Keyboard: {
              [pressed ? "KeyPress" : "KeyRelease"]: {
                key: key,
              },
            },
          };

          const packet = {
            device_id: deviceId,
            timestamp: Date.now(),
            events: [event],
          };

          try {
            this.ws.send(JSON.stringify(packet));
            console.log(`${key} ${pressed ? "pressed" : "released"}`);
          } catch (error) {
            console.error("failed to send ws message:", error);
          }
        }
      }
    }

    this.lastKeyStates = [...keyStates];
    return true;
  }

  handleFeedbackPacket(packet) {
    for (const event of packet.events) {
      if (event.Led && event.Led.Set) {
        this.handleLedEvent(event.Led.Set);
      }
    }
  }

  handleLedEvent(ledEvent) {
    const { led_id, on, brightness, rgb } = ledEvent;
    const cell = this.getCellByLedId(led_id);
    if (cell) {
      this.applyLedToCell(cell, on, brightness, rgb);
    }
  }

  getCellByLedId(ledId) {
    const airSensors = document.querySelectorAll(
      '[data-cell-section="air-sensor"] .grid-cell',
    );
    const sliderButtons = document.querySelectorAll(
      '[data-cell-section="slider"] .grid-cell',
    );

    if (ledId >= 0 && ledId < 16) {
      return sliderButtons[ledId] || null;
    } else if (ledId >= 16 && ledId < 22) {
      const airIndex = ledId - 16;
      return airSensors[airIndex] || null;
    }

    return null;
  }

  applyLedToCell(cell, on, brightness, rgb) {
    if (!on) {
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

    if (brightness !== null && brightness !== undefined) {
      const alpha = brightness / 255;
      color = `rgba(${r}, ${g}, ${b}, ${alpha})`;
    }

    cell.setAttribute("data-rgb-color", color);
    cell.style.backgroundColor = color;

    const glowColor = `rgba(${r}, ${g}, ${b}, 0.6)`;
    cell.style.boxShadow = `inset 0 0 20px ${glowColor}`;
    cell.classList.add("rgb-active");
  }

  getDeviceNameForCell(cell) {
    const container = cell.closest("[data-cell-section]");
    if (container) {
      const section = container.getAttribute("data-cell-section");
      return `chunitroller-${section}`;
    }
    return "chunitroller-webapp";
  }

  close() {
    if (this.ws) {
      this.ws.close();
      this.isConnected = false;
    }
  }
}

class GridController {
  constructor() {
    this.webSocketHandler = new WebSocketHandler();
    this.webSocketHandler.lastKeyStates = [];

    this.cells = [];
    this.compiledCells = [];
    this.keyStates = [];
    this.lastKeyStates = [];

    this.overlapThreshold = 15;
    this.touchCounter = null;

    this.init();
  }

  init() {
    this.cells = document.querySelectorAll(".grid-cell");
    this.touchCounter = document.getElementById("touchCounter");

    this.cells.forEach((cell, index) => {
      cell.setAttribute("data-cell-index", index);
    });

    this.compileKeys();
    this.setupEventHandlers();
    this.updateTouchCounter();

    console.log(`Grid controller initialized with ${this.cells.length} cells`);
  }

  compileKeys() {
    this.compiledCells = [];
    this.keyStates = [];
    this.lastKeyStates = [];

    for (let i = 0; i < this.cells.length; i++) {
      const cell = this.cells[i];
      const prev = cell.previousElementSibling;
      const next = cell.nextElementSibling;

      const compiled = {
        index: i,
        top: cell.offsetTop,
        bottom: cell.offsetTop + cell.offsetHeight,
        left: cell.offsetLeft,
        right: cell.offsetLeft + cell.offsetWidth,
        almostLeft: prev ? cell.offsetLeft + cell.offsetWidth / 4 : -99999,
        almostRight: next
          ? cell.offsetLeft + (cell.offsetWidth * 3) / 4
          : 99999,
        prevIndex: prev ? parseInt(prev.getAttribute("data-cell-index")) : null,
        nextIndex: next ? parseInt(next.getAttribute("data-cell-index")) : null,
        ref: cell,
        section: cell
          .closest("[data-cell-section]")
          ?.getAttribute("data-cell-section"),
        isAirSensor:
          cell
            .closest("[data-cell-section]")
            ?.getAttribute("data-cell-section") === "air-sensor",
      };

      this.compiledCells.push(compiled);
      this.keyStates.push(0);
      this.lastKeyStates.push(0);
    }

    this.webSocketHandler.lastKeyStates = [...this.lastKeyStates];
  }

  isInside(x, y, compiledCell) {
    return (
      compiledCell.left <= x &&
      x < compiledCell.right &&
      compiledCell.top <= y &&
      y < compiledCell.bottom
    );
  }

  getCell(x, y) {
    for (let i = 0; i < this.compiledCells.length; i++) {
      if (this.isInside(x, y, this.compiledCells[i])) {
        return this.compiledCells[i];
      }
    }
    return null;
  }

  setKey(keyStates, index, value = 1) {
    if (index >= 0 && index < keyStates.length) {
      keyStates[index] = value;
    }
  }

  updateTouches(e) {
    try {
      e.preventDefault();

      const newKeyStates = new Array(this.keyStates.length).fill(0);

      for (let i = 0; i < e.touches.length; i++) {
        const touch = e.touches[i];
        const x = touch.clientX;
        const y = touch.clientY;

        const cell = this.getCell(x, y);
        if (!cell) continue;

        this.setKey(newKeyStates, cell.index);

        if (!cell.isAirSensor) {
          if (x < cell.almostLeft && cell.prevIndex !== null) {
            this.setKey(newKeyStates, cell.prevIndex);
          }

          if (x > cell.almostRight && cell.nextIndex !== null) {
            this.setKey(newKeyStates, cell.nextIndex);
          }
        }
      }

      for (let i = 0; i < this.compiledCells.length; i++) {
        const cell = this.compiledCells[i];
        if (newKeyStates[i] !== this.lastKeyStates[i]) {
          if (newKeyStates[i]) {
            cell.ref.classList.add("active");
          } else {
            cell.ref.classList.remove("active");
          }
        }
      }

      if (JSON.stringify(newKeyStates) !== JSON.stringify(this.lastKeyStates)) {
        this.throttledSendKeys(newKeyStates);
      }

      this.keyStates = newKeyStates;
      this.lastKeyStates = [...newKeyStates];
      this.updateTouchCounter();
    } catch (err) {
      console.error("error in updateTouches:", err);
    }
  }

  sendKeys(keyStates) {
    this.webSocketHandler.sendKeyEvents(keyStates);
  }

  setupEventHandlers() {
    const throttledUpdateTouches = throttle((e) => this.updateTouches(e), 10);
    this.throttledSendKeys = throttle(
      (keyStates) => this.sendKeys(keyStates),
      10,
    );

    const container = document.getElementById("main") || document.body;

    container.addEventListener("touchstart", throttledUpdateTouches, {
      passive: false,
    });
    container.addEventListener("touchmove", throttledUpdateTouches, {
      passive: false,
    });
    container.addEventListener("touchend", throttledUpdateTouches, {
      passive: false,
    });
    container.addEventListener("touchcancel", throttledUpdateTouches, {
      passive: false,
    });

    container.addEventListener("mousedown", (e) => {
      if (e.target.classList.contains("grid-cell")) {
        e.preventDefault();
        this.updateTouches({
          preventDefault: () => {},
          touches: [{ clientX: e.clientX, clientY: e.clientY }],
        });
      }
    });

    container.addEventListener("mouseup", (e) => {
      this.updateTouches({
        preventDefault: () => {},
        touches: [],
      });
    });

    container.addEventListener("mouseleave", (e) => {
      this.updateTouches({
        preventDefault: () => {},
        touches: [],
      });
    });

    window.addEventListener("resize", () => {
      this.compileKeys();
    });
  }

  updateTouchCounter() {
    if (this.touchCounter) {
      const activeCount = this.keyStates.filter((state) => state).length;
      this.touchCounter.textContent = `active keys: ${activeCount}`;
    }
  }

  resetAll() {
    this.keyStates.fill(0);
    this.lastKeyStates.fill(0);
    this.webSocketHandler.lastKeyStates = [...this.lastKeyStates];

    this.cells.forEach((cell) => {
      cell.classList.remove("active");
    });

    this.updateTouchCounter();
    console.log("all states reset");
  }

  destroy() {
    this.resetAll();
    this.webSocketHandler.close();
    console.log("GridController destroyed");
  }
}

window.addEventListener("DOMContentLoaded", () => {
  window.gridController = new GridController();

  window.resetGrid = () => {
    window.gridController.resetAll();
  };

  console.log("grid controller loaded");
});

document.addEventListener("contextmenu", (e) => {
  if (e.target.classList.contains("grid-cell")) {
    e.preventDefault();
  }
});