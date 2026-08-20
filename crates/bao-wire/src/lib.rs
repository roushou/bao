//! Bao wire: the transport — length-prefixed JSON framing and the typed
//! client connection.

pub mod client;
pub mod error;
pub mod frame;

pub use client::{Conn, ConnWriter, HostMsg};
pub use error::Error;
pub use frame::{FrameReader, FrameWriter};
