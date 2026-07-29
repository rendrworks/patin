# Roadmap

Patin grows in visible, testable stages. Visible shell features are examples
and templates that validate toolkit APIs; they are not automatically
instantiated by the library:

1. **Foundation** — establish the pinned Rust project, documentation, checks,
   license, and publishing automation.
2. **First surface** — connect as a Wayland client, create a top layer-shell
   surface, reserve its exclusive zone, and fill it with a solid color.
3. **Rendering** — introduce drawing primitives and a correctly scaled clock
   using `tiny-skia` and `cosmic-text`.
4. **Input** — handle pointer and multitouch input; tapping a visible target
   changes bar state without affecting application focus.
5. **UI core** — add internal row, column, and stack layout, styling,
   hit-testing, damage tracking, and reusable components.
6. **Service adapters** — design optional provider interfaces from demonstrated
   battery, network, audio, notification, and media examples.
7. **Session lock** — provide an independently launched, multi-output
   `ext-session-lock-v1` composition with physical/touch password entry and
   PAM authentication.
8. **Composition templates** — exercise phone navigation, launchers, quick
   settings, notifications, and keyboard control without making them defaults.
9. **Compositor integration** — add a replaceable adapter for 0xin workspace
   state and commands.

CPU rendering comes first. A GPU renderer is considered only after measurement
shows a real performance need.
