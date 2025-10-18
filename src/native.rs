use crate::{
    model::Policy,
    store::{Connection, Store},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

pub const HOST_NAME: &str = "com.websiteblocker.companion";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    protocol: u32,
    command: String,
}

#[derive(Serialize)]
struct Response {
    protocol: u32,
    policy: Option<Policy>,
    error: Option<String>,
}

pub fn extension_origin(value: &str) -> bool {
    value
        .strip_prefix("chrome-extension://")
        .and_then(|v| v.strip_suffix('/'))
        .is_some_and(|id| id.len() == 32 && id.bytes().all(|c| (b'a'..=b'p').contains(&c)))
}

pub fn serve(mut input: impl Read, mut output: impl Write, store: &Store) -> Result<(), String> {
    // sendNativeMessage starts one short-lived process per request. Never emit logs to stdout.
    let mut header = [0u8; 4];
    input.read_exact(&mut header).map_err(|e| e.to_string())?;
    let size = u32::from_ne_bytes(header) as usize;
    if size == 0 || size > 4096 {
        return Err("Invalid native message length.".into());
    }
    let mut body = vec![0u8; size];
    input.read_exact(&mut body).map_err(|e| e.to_string())?;
    let result = (|| {
        let request: Request = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
        if request.protocol != 1 || request.command != "snapshot" {
            return Err("Unsupported companion protocol or command.".into());
        }
        let state = store.load()?;
        let now = Utc::now();
        let policy = state.policy(now);
        store.mark_connection(Connection {
            at: now.timestamp(),
            revision: state.revision,
        })?;
        Ok(policy)
    })();
    let response = match result {
        Ok(policy) => Response {
            protocol: 1,
            policy: Some(policy),
            error: None,
        },
        Err(error) => Response {
            protocol: 1,
            policy: None,
            error: Some(error),
        },
    };
    let body = serde_json::to_vec(&response).map_err(|e| e.to_string())?;
    if body.len() > 1_000_000 {
        return Err("Compiled policy exceeds the native message size limit.".into());
    }
    output
        .write_all(&(body.len() as u32).to_ne_bytes())
        .and_then(|_| output.write_all(&body))
        .and_then(|_| output.flush())
        .map_err(|e| e.to_string())
}
