# Hacking Backflow

## Prerequisites

- Some JavaScript knowledge (if you want to hack the frontend)
- Familiarity with (async) Rust (if you want to hack the backend)
- A working Rust toolchain (see [Rustup](https://rustup.rs/))

## Getting Started

1. Clone the repository:

   ```bash
   git clone https://github.com/FyraLabs/backflow.git
   cd backflow
   ```

2. Build the project:

   ```bash
   cargo build
   ```

3. Run the daemon:

   ```bash
    cargo run
   ```

4. Access the web UI at `http://localhost:8000/`

## Adding New Hardware Support

Currently, Backflow only supports the WebSocket remote routing backend. To add support for new hardware, you will need to implement a new input backend.

You may implement one by creating a new module in the `src/backends/` directory. Each backend should implement the `InputBackend` trait defined in `src/backends/mod.rs`.

Your backend should handle communication with the hardware and emit `InputEventPacket` instances to the shared event stream.

## Contributing

Thank you for your interest in contributing to Backflow! We especially welcome:

- New input backends (e.g. RS232 devices, MIDI, HID-over-USB, analog, etc.)
- Enhancements to the WebSocket or PWA frontend
- Improvements to the overall UX, developer experience, or docs

If you're implementing support for a proprietary protocol, please open an issue first with:

- A brief description of the device and its protocol
- Any available documentation or packet dumps
- Notes on how it’s used or how you'd like it to behave

---

## Proprietary RS232-based Arcade I/O Devices

Backflow's initial focus is on supporting arcade-style controllers that communicate via RS232 — often behind USB-to-serial adapters in arcade cabinets.

If you have access to one of these devices, **packet dumps, logs, or protocol documentation are highly appreciated**. We're especially looking to reverse-engineer uncommon or undocumented protocols so they can be supported and remapped by Backflow + InputPlumber.

## CHUNITHM-style Controllers

For those out of the loop, [CHUNITHM](https://en.wikipedia.org/wiki/Chunithm) is a vertical-scrolling rhythm game by Sega, part of their Performai series (alongside maimai and O.N.G.E.K.I). The cabinet uses a unique controller combining:

- A 32-zone capacitive touchpad
- 6 IR motion sensors (air notes)
- a RS232 interface connecting to the cabinet internally

These are typically connected internally via USB-to-serial adapters and polled periodically by the game software.

> [!TIP]
> 💡 Think of [Project SEKAI](https://en.wikipedia.org/wiki/Hatsune_Miku:_Colorful_Stage!) as a mobile-optimized, gesture-based cousin of CHUNITHM — sharing the same DNA but with simplified input tailored for touchscreens.
>
> [YouTube](https://youtu.be/kIAqag8NQAc)

Backflow includes a web-based PWA that mimics the CHUNITHM keyboard, similar to [brokenithm-kb](https://github.com/4yn/brokenithm-kb), and routes its input via WebSocket to Backflow’s virtual device. This makes it easy to test the entire input stack — from your browser to InputPlumber or your own evdev-based backend — without real hardware.

Open-source reimplementations like [slidershim](https://github.com/4yn/slidershim) exist, and Backflow draws heavily from those, but generalizes the concept to cover any nonstandard input scheme — not just CHUNITHM, and also runs on Linux unlike slidershim which is Windows-only.

## Sound Voltex Controllers

[Sound Voltex](https://en.wikipedia.org/wiki/Sound_Voltex) is another arcade VSRG developed by Konami, part of their **BEMANI** series (Most well known for Dance Dance Revolution, Beatmania IIDX, and Pop'n Music). It features a 6-button layout and 2 rotary encoder knobs for low and high-pass filters, and two bottom buttons for activating stutter effects.

```text
+-------------------+
|  [LP]        [HP] |   ← Rotary knobs (Low/High-Pass)
|  [1] [2] [3] [4]  |   ← Standard BT buttons
|   [=S1=] [=S2=]   |   ← FX buttons (lower row)
+-------------------+
```

Input breakdown:

- 4 main BT buttons (digital)
- 2 wide FX buttons (digital)
- 2 rotary encoders (LP/HP) — infinite-turn knobs for audio filters

> [!NOTE]
> The knobs emit relative deltas (clockwise/counterclockwise), and are excellent candidates for `REL_X`/`REL_Y` events. You can even use them as 2D pointers for Etch-a-Sketch-style cursor input.

todo: finish up this section

---

## 🧩 How to Help

If you want to contribute:

- Share packet dumps or documentation for obscure I/O devices (RS232, SPI, HID-over-USB, etc. even just analog pedals)
- Test input backends and remotes with real software
- Help implement real-time mappers for rotary encoders, matrix keyboards, or IR arrays
- Provide feedback on UX, DX and overall accessibility

This not only preserves access for owners of specialized hardware (e.g. arcade controllers, MIDI rigs), but also empowers users with limited mobility, neurodivergence, or accessibility needs to repurpose familiar or adaptive devices into fully remappable, standard input methods.
