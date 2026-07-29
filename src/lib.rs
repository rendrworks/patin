//! Internal Rust toolkit primitives for building Wayland graphical shells.
//!
//! Patin does not instantiate a bar, launcher, service, or device profile.
//! Consumers provide a [`platform::Shell`] implementation and choose their own
//! UI composition. See `examples/demo_bar.rs` for a demonstrator.

pub mod platform;
pub mod render;
pub mod service;
pub mod ui;
