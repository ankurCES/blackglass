//! Length-prefixed JSON-RPC over Unix domain socket. See ADR 0001.
//!
//! Frame format: 4-byte big-endian length prefix, then payload bytes.
//! Max payload: 1 MiB (refuses larger to bound memory).

pub const MAX_FRAME: usize = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("frame too large: {size} > {max}")]
    TooLarge { size: usize, max: usize },
    #[error("short read: need {need} bytes, got {got}")]
    Short { need: usize, got: usize },
}

pub fn encode_frame(payload: &[u8]) -> Vec<u8> {
    let len = (payload.len() as u32).to_be_bytes();
    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&len);
    out.extend_from_slice(payload);
    out
}

pub fn decode_frame(buf: &[u8]) -> Result<(&[u8], &[u8]), FrameError> {
    if buf.len() < 4 {
        return Err(FrameError::Short { need: 4, got: buf.len() });
    }
    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if len > MAX_FRAME {
        return Err(FrameError::TooLarge { size: len, max: MAX_FRAME });
    }
    if buf.len() < 4 + len {
        return Err(FrameError::Short { need: 4 + len, got: buf.len() });
    }
    Ok((&buf[4 + len..], &buf[4..4 + len]))
}

pub mod rpc {
    use serde::{Deserialize, Serialize};
    use serde_json::Value;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Request {
        pub id: u64,
        pub method: String,
        #[serde(default)]
        pub params: Value,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Response {
        pub id: u64,
        pub ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub result: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub error: Option<String>,
    }
}
