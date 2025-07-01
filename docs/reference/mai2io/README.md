# mai2io Reference Code

This directory is part of the [segatools](https://gitea.tendokyu.moe/TeamTofuShop/segatools) project.  
All code here is licensed under the [Unlicense](https://unlicense.org/), with rights reserved to the original authors.

These files are included for **reference purposes only**, serving as a reference implementation for the `mai2io` input backend in Backflow.

## Overview

[maimai](https://en.wikipedia.org/wiki/Maimai_(video_game)) is an arcade rhythm game developed by Sega, recognized for its unique circular playfield and physical outer-button layout. It belongs to the **Performai** series of arcade games alongside CHUNITHM and O.N.G.E.K.I.

The cabinet’s input system includes:

- A large 1:1 touchscreen within a circular frame — present in maimai DX and later models (earlier models lack a touchscreen)
- 8 physical buttons arranged radially around the screen
- A proprietary, polled serial protocol communicated over USB-to-RS232 adapters

Each cabinet typically includes **two full input sets** (touchscreen + 8 buttons), configured for simultaneous two-player gameplay.

> 💡 **Note:** maimai cabinets are usually set up with an extended multi-display configuration under Windows, with each player's screen rendered as a separate display output.

Backflow aims to provide a **modern userspace driver** for maimai controllers and cabinets, enabling input mapping and remapping on Linux without relying on closed-source Windows drivers or SegaTools patches.

## Hardware Testing

If you're interested in testing with real hardware or building a compatible controller:

- [**Rhythm Cons Wiki – maimai**](https://rhythm-cons.wiki/controllers/maimai/maimai/) – A community-curated page with schematics, device documentation, and firmware links
