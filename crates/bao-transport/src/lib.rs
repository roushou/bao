//! Bao transport: the plumbing both ends share — length-prefixed framing and
//! addressing. No Bao domain or protocol types here; this crate only knows
//! how bytes move over a stream.

pub mod addr;
pub mod error;
pub mod frame;

pub use addr::{Addr, DEFAULT_PORT};
pub use error::Error;
pub use frame::{FrameReader, FrameWriter};
