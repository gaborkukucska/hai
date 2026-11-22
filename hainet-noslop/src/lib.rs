//! START OF FILE hainet-noslop/src/lib.rs
//!
//! # hainet-noslop
//!
//! The NoSlop mesh protocol as the HAI-Net Home HUB needs it, matched byte for byte to the legacy
//! Android app (NoSlop 0.8.x, `com.noslop.app.mesh` / `com.noslop.app.crypto`):
//!
//! * [`json`]: order-preserving JSON rendered exactly like Gson (for the envelope digest).
//! * [`canon`]: `encodeForSigning` (UTF-16 lengths) and every canonical signed string the Hub uses.
//! * [`keys`]: X.509 / PKCS#8 / raw key formats, onion derivation, Tor secret key file, signing.
//! * [`packet`] and [`envelope`]: `NetworkPacket` and the `noslop-env-v1` envelope signature.
//! * [`dm`]: DM v2 (directional HKDF + AAD) and legacy v1 decryption.
//! * `hub` (feature `hub`): the Home HUB node that hosts the owner's creator identity, keeps a durable
//!   inbox for the phone, and serves the owner's control API.
//!
//! Every format is checked against vectors produced by the app's own libraries (Gson 2.10,
//! Bouncy Castle 1.78.1): `tests/vectors/noslop_vectors.json`, generator in `tests/vector-generator/`.

pub mod canon;
pub mod dm;
pub mod envelope;
pub mod json;
pub mod keys;
pub mod packet;

#[cfg(feature = "hub")]
pub mod hub;
