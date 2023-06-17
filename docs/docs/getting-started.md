---
sidebar_position: 3
---

# Getting Started

Grebuloff is currently in a very early stage of development, and is not yet ready for general use. However, if you are
interested in contributing to the project, or just want to try it out, this guide will help you get started.

## System Requirements

:::info
Rust nightly is required due to the use of the [dll-syringe](https://crates.io/crates/dll-syringe) crate,
which depends on unstable Rust features.
:::

:::note
Rust nightly versions newer than `nightly-2023-06-02` currently fail to build Grebuloff due to an
[issue](https://github.com/denoland/rusty_v8/issues/1248) in the V8 bindings that Grebuloff uses.

This issue will be resolved once a new version of the V8 bindings is released on crates.io.
:::

- [Node.js](https://nodejs.org/) 16+
  - Use of the latest LTS version is recommended.
  - [pnpm](https://pnpm.io/) is also required.
- [Rust](https://www.rust-lang.org/) 1.72.0-nightly-2023-06-02
  - Install using ```rustup toolchain install nightly-2023-06-02```
- [Visual Studio 2022](https://visualstudio.microsoft.com/vs/) (with C++ and .NET Desktop Development Workloads)

## Boneless

Grebuloff uses a custom build script called Boneless. Boneless is a purpose-built (read: fully jank) build system
that is designed to pull together all of the different moving parts of Grebuloff into a single build process.
It's not pretty, but it works (most of the time).

### Building

:::tip
The command examples in this documentation assume you are running from a POSIX-like environment
on Windows, such as Git Bash. If you aren't, you may need to replace forward slashes with backslashes,
i.e. `./boneless` becomes `.\boneless`.
:::

Boneless is exposed as a CLI tool in the root of the repository. To build all of the required components of Grebuloff,
run the following command:

```shell
$ ./boneless build
Found rustc 1.72.0-nightly
Found .NET 7.0.302

(...)
```

Boneless will check your build environment for the required tools and dependencies, and will build all of the
required components of Grebuloff. This process can take a while, especially the first time you run it.

:::caution
Building Grebuloff requires several gigabytes of free disk space. It is also recommended to have a fast
internet connection, as several heavy dependencies, including a prebuilt copy of V8, will be downloaded.
:::

### Running

Once the build process has completed, you can fake-launch the game and inject Grebuloff into it using the
`launch` command:

```shell
./boneless launch
```

Note that, on the first launch, you will need to set your game path before running this command.
You can set your game path using the `set-path` command:

```shell
./boneless set-path "/path/to/FINAL FANTASY XIV - A Realm Reborn/game/ffxiv_dx11.exe"
```

If all goes well, the game will launch, and... nothing will appear to happen. That's a good thing!
Check for the existence of a `grebuloff.log` file in the `build` directory. If it exists, Grebuloff
is running in your game!
