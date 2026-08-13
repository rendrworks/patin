//! Consumer-facing configuration types: how a surface is placed and what
//! kinds of input it accepts. Pure data — no Wayland objects — so a
//! consumer can build and unit-test its configuration without a
//! compositor connection.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayerLevel {
    Background,
    Bottom,
    Top,
    Overlay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyboardPolicy {
    None,
    Exclusive,
    OnDemand,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Anchors {
    pub top: bool,
    pub bottom: bool,
    pub left: bool,
    pub right: bool,
}

pub struct LayerConfig {
    pub namespace: String,
    pub layer: LayerLevel,
    pub anchors: Anchors,
    pub size: (u32, u32),
    pub exclusive_zone: i32,
    pub keyboard: KeyboardPolicy,
    pub visibility: LayerVisibility,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayerVisibility {
    /// Always visible; no external toggle.
    Fixed,
    /// SIGUSR1 hides the surface (unmapping it and releasing its exclusive
    /// zone) and SIGUSR2 shows it again — the same convention wvkbd uses,
    /// so a compositor's existing gesture/text-input show/hide hooks work
    /// unchanged. Opt-in per surface: most shells should not be
    /// dismissable by an external signal (a lock screen must never be).
    ToggleBySignal { start_visible: bool },
}

pub struct WindowConfig {
    pub app_id: String,
    pub title: String,
    pub initial_size: (u32, u32),
    pub min_size: Option<(u32, u32)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextInputPurpose {
    Normal,
    Password,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KeyInput {
    Text(String),
    Backspace,
    Enter,
    Escape,
}

/// A synthetic key event for `virtual-keyboard-v1` injection: an `evdev`-
/// style wire keycode, plus any real XKB modifiers (`ControlMask`,
/// `Mod1Mask`, i.e. Alt) that should be held for it. These are the XKB
/// *real* modifiers — fixed core positions every keymap has regardless of
/// its own `xkb_types`, so no keymap changes are needed to use them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualKey {
    pub keycode: u32,
    pub modifiers: u32,
}

impl VirtualKey {
    pub const SHIFT: u32 = 1 << 0;
    pub const CONTROL: u32 = 1 << 2;
    pub const ALT: u32 = 1 << 3;
}
