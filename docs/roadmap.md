# Roadmap

Patin grows in visible, testable stages:

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
6. **Services** — expose battery, network, audio, notifications, and media
   state through standard system interfaces.
7. **Mobile profile** — add opt-in phone navigation, launcher, quick settings,
   notifications, and keyboard control.
8. **Compositor integration** — add a replaceable adapter for 0xin workspace
   state and commands.

CPU rendering comes first. A GPU renderer is considered only after measurement
shows a real performance need.

