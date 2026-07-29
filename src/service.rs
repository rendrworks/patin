//! Interface shared by optional, out-of-tree service adapters.
//!
//! Patin never constructs a provider itself. A consumer builds one from an
//! adapter crate such as `patin-service-upower`, stores it, and polls it on
//! whatever schedule its `Shell::update` already uses.

pub trait Provider {
    type Snapshot: Clone + PartialEq;

    fn poll(&mut self) -> Self::Snapshot;
}
