# chuniio reference code

This directory is part of the [segatools](https://gitea.tendokyu.moe/TeamTofuShop/segatools) project.
All the code here is under the [Unlicense](https://unlicense.org/). Rights reserved to the original authors.

These files are included for **reference purposes only**, serving as a reference implementation for the `chuniio` Backflow input backend.

## Overview

[CHUNITHM](https://en.wikipedia.org/wiki/Chunithm) is an arcade vertical-scrolling rhythm game (VSRG) developed by Sega. It features a unique control scheme using:

- A 32-zone capacitive touch slider (16 zones are only used in-game, either of the 2 rows can be activated as a single zone)
- 6 IR-based vertical motion sensors (air notes)

Many custom and commercial CHUNITHM controllers exist — often outputting data over RS232 using segatools' serial shim protocol. Backflow aims to provide a **modern userspace driver** for these devices, allowing them to work on Linux **without vendor lock-in or Windows-only shims**.

## About the Protocol

These controllers typically communicate over RS232 or USB-serial bridges using a polling-based protocol. We plan to integrate protocol parsing directly into Backflow, enabling real-time event mapping to virtual devices (via uinput or InputPlumber targets).

### Hardware Testing

If you would like to build or get your hands on a controller, there are some resources available:

- [chu-pico/rpunithm schematics and BOM](https://github.com/whowechina/chu_pico) - These replaces the 6 IR sensors with ToF sensors, allowing for a more compact but less accurate design.
- [Laverita 3](https://yuancon.store/controller/laverita-3) - A commercial controller by Yuancon, natively outputting segatools' serial shim protocol.
- [Tasoller](https://www.dj-dao.com/en/tasoller) - A commercial controller by DJ-DAO, with custom firmware support for outputting segatools' serial shim protocol.
- [Rhythm Cons Wiki - CHUNITHM](https://rhythm-cons.wiki/controllers/chunithm/chunithm/) - Documentation on how to build your own CHUNITHM controller, with community-provided schematics and BOMs.

> If you own one of these controllers — or have packet dumps, documentation, or firmware — please consider sharing it to help us expand support!
