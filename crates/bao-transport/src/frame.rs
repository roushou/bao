//! Length-prefixed JSON framing: one message per frame (u32 big-endian
//! length, then the JSON bytes). `FrameReader`/`FrameWriter` own an I/O half
//! and are bound to the message type they carry.

use std::{io, marker::PhantomData};

use serde::{Serialize, de::DeserializeOwned};

use crate::error::Error;

/// A frame (length prefix included) may not exceed this many bytes.
const MAX_FRAME: usize = 64 * 1024 * 1024;

/// One direction of the wire: reads length-prefixed JSON frames from a
/// stream and decodes them as `M`. A clean EOF (peer closed between frames)
/// is `Ok(None)`; a partial, oversized, or undecodable frame is `Err`.
pub struct FrameReader<R, M> {
    stream: R,
    _msg: PhantomData<M>,
}

impl<R: tokio::io::AsyncRead + Unpin, M: DeserializeOwned> FrameReader<R, M> {
    pub fn new(stream: R) -> Self {
        FrameReader {
            stream,
            _msg: PhantomData,
        }
    }

    /// One message, or `None` once the peer has closed cleanly.
    pub async fn read(&mut self) -> Result<Option<M>, Error> {
        match read_frame(&mut self.stream).await? {
            Some(bytes) => Ok(Some(decode(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Recover the underlying stream (e.g. to re-bind it after the channel
    /// handshake).
    pub fn into_inner(self) -> R {
        self.stream
    }
}

/// The other direction of the wire: encodes `M` as JSON, frames it, and
/// writes it to a stream. Serialize failures surface as `Error::Json` —
/// never a panic.
pub struct FrameWriter<W, M> {
    stream: W,
    _msg: PhantomData<M>,
}

impl<W: tokio::io::AsyncWrite + Unpin, M: Serialize> FrameWriter<W, M> {
    pub fn new(stream: W) -> Self {
        FrameWriter {
            stream,
            _msg: PhantomData,
        }
    }

    /// Encode, frame, write, flush.
    pub async fn write(&mut self, msg: &M) -> Result<(), Error> {
        write_frame(&mut self.stream, &encode(msg)?).await
    }

    /// Recover the underlying stream (e.g. to re-bind it as another message
    /// type after the channel handshake).
    pub fn into_inner(self) -> W {
        self.stream
    }
}

/// One frame: a u32 big-endian length, then the bytes. Flushes per frame.
async fn write_frame<W: tokio::io::AsyncWrite + Unpin>(
    w: &mut W,
    bytes: &[u8],
) -> Result<(), Error> {
    use tokio::io::AsyncWriteExt;
    let len = bytes.len() as u32;
    w.write_all(&len.to_be_bytes()).await?;
    w.write_all(bytes).await?;
    w.flush().await?;
    Ok(())
}

/// One frame: read the length, then the payload. `Ok(None)` on clean EOF.
async fn read_frame<R: tokio::io::AsyncRead + Unpin>(r: &mut R) -> Result<Option<Vec<u8>>, Error> {
    use tokio::io::AsyncReadExt;
    let mut lenb = [0u8; 4];
    match r.read_exact(&mut lenb).await {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let len = u32::from_be_bytes(lenb) as usize;
    if len > MAX_FRAME {
        return Err(Error::FrameTooLarge(len));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    Ok(Some(buf))
}

fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, Error> {
    Ok(serde_json::to_vec(msg)?)
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    Ok(serde_json::from_slice(bytes)?)
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use tokio::io::{AsyncWriteExt, duplex};

    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Msg {
        id: u32,
    }

    #[tokio::test]
    async fn round_trips_typed_messages() {
        let (a, b) = duplex(4096);
        let mut writer = FrameWriter::<_, Msg>::new(a);
        let mut reader = FrameReader::<_, Msg>::new(b);

        writer.write(&Msg { id: 7 }).await.unwrap();

        let got = reader.read().await.unwrap().expect("one message");
        assert_eq!(got, Msg { id: 7 });
    }

    #[tokio::test]
    async fn clean_eof_is_none() {
        let (a, b) = duplex(4096);
        let mut writer = FrameWriter::<_, Msg>::new(a);
        let mut reader = FrameReader::<_, Msg>::new(b);

        writer.write(&Msg { id: 1 }).await.unwrap();
        drop(writer); // close the write side

        assert!(reader.read().await.unwrap().is_some());
        assert!(reader.read().await.unwrap().is_none(), "clean EOF -> None");
    }

    #[tokio::test]
    async fn oversized_frame_is_rejected() {
        let (a, mut b) = duplex(4096);
        let mut reader = FrameReader::<_, Msg>::new(a);

        // A length header beyond the cap, with no payload to follow.
        let len = (MAX_FRAME + 1) as u32;
        b.write_all(&len.to_be_bytes()).await.unwrap();

        assert!(matches!(
            reader.read().await.unwrap_err(),
            Error::FrameTooLarge(_)
        ));
    }

    #[tokio::test]
    async fn corrupt_frame_is_rejected() {
        let (a, mut b) = duplex(4096);
        let mut reader = FrameReader::<_, Msg>::new(a);

        let payload = b"{ not json";
        b.write_all(&(payload.len() as u32).to_be_bytes())
            .await
            .unwrap();
        b.write_all(payload).await.unwrap();

        assert!(matches!(reader.read().await.unwrap_err(), Error::Json(_)));
    }
}
