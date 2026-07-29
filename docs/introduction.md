# Introduction

Patin is a native Rust toolkit for building Wayland graphical shells. It
provides focused shell infrastructure; a consuming project decides which bars,
overlays, launchers, notifications, lock screens, or navigation surfaces exist.

Patin-powered shells are clients, not compositors. The toolkit is tested
alongside
[0xin](https://github.com/termworks/0xin), but uses standard Wayland protocols
so its core shell remains useful with other layer-shell compositors. When 0xin
gains a control socket, Patin will access compositor-specific workspace state
and commands through a replaceable adapter.

## Toolkit boundary

The library owns Wayland layer surfaces, scaling, seats, pointer/touch routing,
shared-memory rendering, logical layout primitives, draw commands, and damage
submission. It does not instantiate a particular shell composition or service.

Examples are executable fixtures. The demo bar intentionally contains a clock
and optional battery, volume, brightness, and network status readers so the
library can be exercised on real systems. Those features are not part of
Patin's default behavior and do not become required dependencies for
consumers.

Patin remains a shell-focused toolkit rather than a general-purpose
application GUI framework, configuration language, or reactive hot-reload
runtime.

The work is split into small visible stages. Every completed stage explains the
concepts it introduces, the files and important functions it changes, and the
commands that actually verified it.
