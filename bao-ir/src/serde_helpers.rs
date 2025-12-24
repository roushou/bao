//! Serde helpers for types that don't impl Serialize by default.

use std::time::Duration;

use serde::{Serialize, Serializer};

/// Serialize an Option<Duration> as Option<milliseconds>.
pub fn serialize_option_duration<S>(
    duration: &Option<Duration>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    duration.map(|d| d.as_millis()).serialize(serializer)
}
