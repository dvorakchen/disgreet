//! core.rs

use std::{
    env,
    io::{Read, Write},
    os::unix::net::UnixStream,
};

use log::debug;
use serde_json::{Value, json};
use thiserror::Error;

use crate::Session;

pub(crate) fn authenticate(
    username: &str,
    password: &str,
    session: &Session,
) -> anyhow::Result<()> {
    debug!("start authentication: username: {username}, password: {password}");

    let sock_path = env::var("GREETD_SOCK").map_err(|_| AuthError::EnvGreetdSock)?;

    let mut client = UnixStream::connect(sock_path)?;

    let msg = json!({
        "type": "create_session",
        "username": username
    });
    send_msg(&mut client, &msg)?;
    debug!("sent create_session: {:?}", msg);

    let resp = recv_msg(&mut client)?;
    debug!("response: {:?}", resp);

    let msg_type = resp["type"].as_str().unwrap_or("");

    if msg_type != "auth_message" {
        let e = resp["description"].as_str().unwrap_or("Unknow Error");
        return Err(AuthError::Username(e.to_string()).into());
    }

    let msg = json!({
        "type": "post_auth_message_response",
        "response": password
    });

    debug!("send post_auth_message_response: {:?}", msg);
    send_msg(&mut client, &msg)?;

    let resp = recv_msg(&mut client)?;
    debug!("response: {:?}", resp);

    let msg_type = resp["type"].as_str().unwrap_or("");
    if msg_type != "success" {
        let e = resp["description"].as_str().unwrap_or("Unknow Error");
        return Err(AuthError::Password(e.to_string()).into());
    }

    debug!("auth success");
    debug!("launch session: {:?}", session);

    // greetd 协议要求 cmd 是字符串数组，例如 ["/usr/bin/bash", "--login"]
    let cmd: Vec<&str> = session.exec.split_whitespace().collect();
    let msg = json!({
        "type": "start_session",
        "cmd": cmd
    });
    debug!("send start_session: {:?}", msg);
    send_msg(&mut client, &msg)?;

    Ok(())
}

#[derive(Debug, Error)]
pub(crate) enum AuthError {
    #[error("cannot found environment GREETD_SOCK")]
    EnvGreetdSock,
    #[error("wrong username: {0}")]
    Username(String),
    #[error("wrong password: {0}")]
    Password(String),
    // #[error("Launch session: {0}")]
    // LaunchSession(String),
}

fn send_msg(stream: &mut UnixStream, msg: &Value) -> anyhow::Result<()> {
    let json_bytes = serde_json::to_vec(msg)?;
    let len = json_bytes.len() as u32;

    stream.write_all(&len.to_ne_bytes())?;
    stream.write_all(&json_bytes)?;
    Ok(())
}

fn recv_msg(stream: &mut UnixStream) -> anyhow::Result<Value> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_ne_bytes(len_buf) as usize;

    let mut json_buf = vec![0u8; len];
    stream.read_exact(&mut json_buf)?;

    let val: Value = serde_json::from_slice(&json_buf)?;

    Ok(val)
}
