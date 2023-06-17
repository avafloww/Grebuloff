---
sidebar_position: 6
---

# Dalamud Support Plugin

> **Language:** C#

The Dalamud Support Plugin is a Dalamud plugin that allows Grebuloff and Dalamud
to run simultaneously. When the plugin is loaded by Dalamud, it will inject the
[low-level runtime](/architecture/llrt) into the game process, starting the
process of bootstrapping Grebuloff.

When Grebuloff has been loaded through the support plugin, as opposed to through
the [injector](/architecture/injector), function hooks will be handled through
Dalamud, to avoid conflicts with other plugins.
