# Backflow

> A networked, extensible, remappable, open-source software-defined input bridge.

**Backflow** is a daemon that bridges unconventional input devices to standard HID events, powered by [InputPlumber](https://github.com/ShadowBlip/InputPlumber). Whether you're using analog controls, serial interfaces, web-based gamepads, MIDI devices, or other custom hardware, Backflow provides a modular way to route and remap any input into standard keyboard, mouse, or gamepad events.

Whether you're building a virtual gamepad in a browser, integrating RS232 or MIDI devices, or enabling hands-free control through custom hardware, Backflow helps unify everything into one virtual input stack.

This makes it ideal not just for gaming or custom hardware, but also for accessibility scenarios. Think of it as the [Xbox Adaptive Controller](https://www.xbox.com/en-US/accessories/controllers/xbox-adaptive-controller) — but entirely in software.

> No hands, no controller? No problem.
> Bring your own input, and play it your way.

## Relation to InputPlumber

Backflow is a companion project to InputPlumber, extending its reach to nonstandard, domain-specific, and experimental input sources — especially where native integration would be impractical or outside the project’s intended scope.

Rather than creating composite devices from other HID devices, Backflow connects to InputPlumber via D-Bus and injects input events into standalone target devices. This enables the full power of InputPlumber’s remapping and transformation engine to be applied to any input source — not just HID-compatible hardware.

This architecture cleanly separates concerns:

- InputPlumber focuses on HID-to-HID remapping, event transformation, and composite device logic
- Backflow creates and manages standalone target devices using InputPlumber’s D-Bus API, injecting real-time input events from sources like USB sniffers, MIDI devices, browser-based UIs, arcade IO boards, or other unconventional interfaces.

Together, they form a flexible and extensible input framework capable of adapting to a wide range of devices and use cases — from gaming to accessibility tooling.

By building on InputPlumber’s userspace HID infrastructure, Backflow enables arbitrary inputs to be remapped, virtualized, and exposed as standard devices — all without kernel drivers or vendor-specific Windows-only shims.

> TL;DR: Backflow is a virtual device bridge for InputPlumber, letting you use any input source as a fully remappable HID device.

## (Planned) Features

- **WebSocket server** for receiving input events
- **PWA virtual gamepad server** using the WebSocket routing backend
- **Modular input backends** with support for:
  - Serial devices (e.g. RS232, UART, GPIO)
  - MIDI instruments and controllers
  - Analog input (e.g. joystick axes, potentiometers, rotary encoders, pedals)
- **RGB feedback output** to various devices and protocols:
  - [JVS](https://en.wikipedia.org/wiki/Japan_Amusement_Machine_and_Marketing_Association#Video)
  - Novation MIDI hardware
  - OpenRGB-compatible devices
- **Input remapping and transformation**, using InputPlumber's configuration and virtual device system

---

## Use Cases

- Virtual touch-based gamepads for PC gaming
- Accessibility solutions for users with limited mobility
- Repurposing old input devices (e.g. old joysticks, proprietary arcade hardware) for other applications
- Virtual gamepad emulation for proprietary control schemes

---

## The origin story

Backflow was actually not initially intended to be a universal input framework.
It started as a Linux-based implementation of [slidershim](https://github.com/4yn/slidershim), a project that allows users to remap their RS232-based CHUNITHM-style controllers to various input schemes.
However, as we started brainstorming the possibilities initially, it became clear that this could be expanded into a much more universal input framework, if marketed correctly and not scoped to just trying to hack $300 arcade pads or iPads to play VSRGs.

And as there seemed to simply be no such solution available already from:

- Steam Input (which only support well HID gamepads)
- Pinnacle Game Profiler (Windows only, proprietary, and dead project literally, the maintainer passed way RIP)
- slidershim (Windows only, scoped only to either RS232-based controllers, PWA virtual gamepads, and limited to just playing VSRGs or Project DIVA)
- Plain old InputPlumber (which is a great app, but only remaps HID events from one device to another, just like Steam Input)

So we decided to build Backflow, an input bridge that converts unconventional inputs into arbitrary D-Bus messages for InputPlumber to consume, remapping them to any HID events supported by InputPlumber.
