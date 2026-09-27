//! # knight-net
//!
//! Networking for Knight Engine games, on every platform the engine runs on.
//!
//! - [`codec`]: a compact binary codec ([`Wire`] trait) for your messages.
//! - [`relay`]: the relay protocol (rooms, peers, payloads). Browsers cannot accept incoming
//!   connections, so web multiplayer goes through a relay; `apps/relay` is one.
//! - [`Client`] + transports: [`connect_ws`] (WebSocket, native and web) and [`LocalHub`]
//!   (in-process: tests, hot seat, bots).
//! - [`Lockstep`]: deterministic lockstep for turn-based and fixed-tick simulations — peers
//!   exchange commands, not state.

pub mod codec;
pub mod lockstep;
pub mod relay;
mod transport;
mod ws;

pub use codec::{DecodeError, Reader, Wire, Writer};
pub use lockstep::Lockstep;
pub use relay::{ClientMsg, PeerId, Rooms, ServerMsg};
pub use transport::{Client, ConnState, LocalHub, NetEvent, Transport, connect_ws};
