---
slug: /
sidebar_position: 1
---

# Introduction

Grebuloff is an experimental addon framework for Final Fantasy XIV. It introduces a new concept of what
plugins can be, focusing on enabling creation of addons that are isolated, secure, stable, and add onto the vanilla
game in an incremental fashion.

The core of Grebuloff is built in Rust and TypeScript. Addons, while typically written in JavaScript or
TypeScript, can be developed using any technology that can run on the V8 engine, including WebAssembly.

## How does Grebuloff relate to Dalamud?

> Grebuloff is currently in a very early stage of development. If you are a new community developer looking
> to make the Next Big Plugin, or an end-user looking for a wide ecosystem of addons for the game,
> **you should use XIVLauncher & Dalamud**.

**Grebuloff is _not_ a replacement for Dalamud.** These projects have entirely different design philosophies.
Grebuloff can even run alongside Dalamud using a helper plugin, allowing you to use both frameworks at the
same time; however, this feature, like everything else, is highly experimental.

Dalamud plugins are able to extensively alter a running game, thanks to an extensive API and, where its API
falls short, the ability to hook game functions and directly modify memory. However, this often can come
at the cost of stability (especially during game patches) and security, as plugins have unscoped, unsandboxed
access to your game and your computer.

Grebuloff is intended to offer a safer, more isolated framework for addons. All addons run in an isolated
V8 context, and only have access to the APIs they have explicitly requested and been granted access to.

It's important to note that, since third-party tools are against Square Enix's Terms of Service, use of either
Grebuloff or Dalamud carries risks of penalties to your account. Although both projects make efforts to mitigate
this risk, the responsibility of account safety ultimately falls upon the user.

## License

Grebuloff is licensed under LGPL-3.0.
[Please refer to the `LICENSE` file for more details.](https://github.com/avafloww/Grebuloff/blob/main/LICENSE)

Dependencies are licensed under their project's respective licenses.
