//! Storage locations shared by host boot configuration and the running storage service.
//! The daemon and admission operations must open the same selected writable overlay.

/// Persistent host-store directory containing retained generations and the active selection.
pub const ROOT: &str = "/var/lib/airlock/host-store";

/// Nix backend for the immutable selected lower store and generation-specific writable layer.
/// The nested local-store URL is encoded because it is a query parameter of the overlay URL.
pub const URI: &str = concat!(
    "local-overlay://?lower-store=",
    "local%3Froot%3D%2Fvar%2Flib%2Fairlock%2Fhost-store%2Flower%26read-only%3Dtrue",
    "&upper-layer=/var/lib/airlock/host-store/active/upper",
);
