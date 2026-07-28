# Introduction

Patin is a native Rust graphical shell for Wayland. It is responsible for the
visible desktop and phone surfaces around applications: bars, overlays,
launchers, notifications, a lock screen, and optional mobile navigation.

Patin is a client, not a compositor. It is designed alongside
[0xin](https://github.com/termworks/0xin), but uses standard Wayland protocols
so its core shell remains useful with other layer-shell compositors. When 0xin
gains a control socket, Patin will access compositor-specific workspace state
and commands through a replaceable adapter.

## Product boundary

The first product is one opinionated shell. Patin will build reusable internal
Rust primitives as its own UI needs them, but will not design a public
general-purpose GUI toolkit, configuration language, or reactive hot-reload
runtime in v1.

The work is split into small visible stages. Every completed stage explains the
concepts it introduces, the files and important functions it changes, and the
commands that actually verified it.

