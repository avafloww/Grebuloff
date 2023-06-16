# Grebuloff

Grebuloff is an experimental addon framework for Final Fantasy XIV. It introduces a new concept of what
plugins can be, focusing on enabling creation of plugins that are isolated, secure, stable, and add onto the vanilla
game in an incremental fashion.

The core of Grebuloff is built in Rust and TypeScript. Plugins, while typically written in JavaScript or
TypeScript, can be developed using any technology that can run on the V8 engine, including WebAssembly.

### How does Grebuloff relate to Dalamud?

**Grebuloff is _not_ a replacement for Dalamud.** These projects have entirely different design philosophies.
Grebuloff can even run alongside Dalamud using a helper plugin, allowing you to use both frameworks at the
same time; however, this feature, like everything else, is highly experimental.

Dalamud plugins are able to extensively alter a running game, thanks to an extensive API and, where its API
falls short, the ability to hook game functions and directly modify memory. However, this often can come
at the cost of stability (especially during game patches) and security, as plugins have unscoped, unsandboxed
access to your game and your computer.

Grebuloff is intended to offer a safer, more isolated framework for plugins. All plugins run in an isolated
V8 context, and only have access to the APIs they have explicitly requested and been granted access to.

It's important to note that, since third-party tools are against Square Enix's Terms of Service, use of either
Grebuloff or Dalamud carries risks of penalties to your account. Although both projects make efforts to mitigate
this risk, the responsibility of account safety ultimately falls upon the user.

## Roadmap

Grebuloff is currently in a very early stage of development. If you are a new community developer looking
to make the Next Big Plugin, or an end-user looking for a wide ecosystem of addons for the game,
**you should use XIVLauncher & Dalamud**.

- [X] Injector that mostly works, for both fake-launch & inject into a running game
- [ ] V8 engine bringup for core components
- [ ] UI bringup using React (as opposed to `MessageBox`)
- [ ] Basic plugin support (including basic game APIs)
- [ ] Game APIs for all the things, ever

## Architecture

### Components

| Component            | Language   | Description                                                                                          |
|----------------------|------------|------------------------------------------------------------------------------------------------------|
| `grebuloff-injector` | Rust       | Injects the runtime into the game process.                                                           |
| `grebuloff-llrt`     | Rust       | Low-level core runtime, injected by the injector. Handles game communications & bootstraps HLRT.     |
| `grebuloff-hlrt`     | TypeScript | High-level core runtime that runs in the privileged isolate.                                         |
| `grebuloff-hlrt-lib` | TypeScript | Present in every isolate, and used to build the V8 snapshot.                                         |
| `grebuloff-ui`       | TypeScript | Provides UI services for Grebuloff and plugins. Written in React, and runs in the WebView2 instance. |
| `grebuloff-dalamud`  | C#         | A Dalamud helper plugin that allows Grebuloff and Dalamud to run simultaneously.                     |

## Credits & Acknowledgements

Without the work of these people and groups, this project would not be possible.

Thanks to:

- [The contributors to Grebuloff](https://github.com/avafloww/Grebuloff/graphs/contributors)
- [goat](https://github.com/goaaats/) and all of the folks at [@goatcorp](https://github.com/goatcorp), for
  their tireless work on creating Dalamud & XIVLauncher, the projects that changed the game and inspired us all
- [aers](https://github.com/aers), [Pohky](https://github.com/Pohky), [Caraxi](https://github.com/Caraxi),
  [daemitus](https://github.com/daemitus),
  and [all of the contributors](https://github.com/aers/FFXIVClientStructs/graphs/contributors)
  to [FFXIVClientStructs](https://github.com/aers/FFXIVClientStructs), for their extensive research into the
  game's internals
- The community developers at [goat place](https://goat.place), also for their extensive research into the
  game's internals, as well as for entertaining my constant memery
- [Deno](https://github.com/denoland/deno) and [MiniV8](https://github.com/SkylerLipthay/mini-v8) for
  providing excellent examples and code to embed V8 in Rust
- Square Enix, for creating the critically acclaimed game that we all know and love

## License

Grebuloff is licensed under LGPL-3.0. Please refer to the `LICENSE` file for more details.

Dependencies are licensed under their project's respective licenses.