---
sidebar_position: 4
---

# High-Level Runtime (HLRT)

> **Language:** TypeScript

The High-Level Runtime (HLRT) is the core of Grebuloff. It runs in a privileged
V8 isolate, and is responsible for much of Grebuloff's core functionality,
including plugin management.

HLRT communicates with the [low-level runtime](/architecture/llrt) to handle
game communications, and with the [UI](/architecture/ui) to provide UI services
to addons.
