//! Minimal capability-free Ping host using the existing bounded framing code.
use super::*;
use serde_json::{Value, json};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    io::{BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
use yapfast_plugin_api::frame::{self, Frame, FrameKind};

struct Helper(Child);
impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn send(output: &mut impl Write, kind: FrameKind, id: u64, payload: Value) -> Result<()> {
    frame::write_frame(output, &Frame::new(kind, id, serde_json::to_vec(&payload)?))?;
    Ok(())
}

fn wait(
    input: &mpsc::Receiver<std::result::Result<Frame, String>>,
    output: &mut impl Write,
    kind: &str,
    id: u64,
) -> Result<Value> {
    let until = Instant::now() + Duration::from_secs(10);
    for _ in 0..128 {
        let frame = input
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .context("Plugin helper did not answer before its deadline")?
            .map_err(anyhow::Error::msg)?;
        let value: Value = serde_json::from_slice(&frame.payload)?;
        if value["t"] == "call" {
            // No chat/token/network/filesystem capability is exposed by this POC.
            send(
                output,
                FrameKind::Reply,
                frame.id,
                json!({"t":"return","plugin":value["plugin"],"call":value["call"],"answer":{"denied":{"reason":"The loader prototype exposes no privileged host capabilities"}}}),
            )?;
        } else if value["t"] == "fault" || value["t"] == "bye" {
            bail!("Plugin helper faulted or exited")
        } else if value["t"] == kind && frame.id == id {
            return Ok(value);
        }
    }
    bail!("Plugin helper exceeded its bounded response count")
}

pub fn ping(config: &Config) -> Result<String> {
    if sha256(&config.helper)? != config.helper_sha256
        || sha256(&config.wasm)? != config.wasm_sha256
    {
        bail!("Runtime helper or plugin failed its launch integrity check")
    }
    let mut cmd = Command::new(&config.helper);
    cmd.arg("run").env_clear();
    // The runtime is an explicit, trusted build. No user tokens or profile env is inherited.
    for key in ["SystemRoot", "WINDIR", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(key) {
            cmd.env(key, value);
        }
    }
    cmd.env("RAYON_NUM_THREADS", "2")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    let mut helper = Helper(
        cmd.spawn()
            .context("Could not start the existing WebAssembly runtime helper")?,
    );
    let mut output = helper.0.stdin.take().context("Missing helper input")?;
    let stdout = helper.0.stdout.take().context("Missing helper output")?;
    let (tx, rx) = mpsc::sync_channel(4);
    let reader = std::thread::spawn(move || {
        let mut input = BufReader::new(stdout);
        loop {
            match frame::read_frame(&mut input) {
                Ok(Some(f)) => {
                    if tx.send(Ok(f)).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                    break;
                }
            }
        }
    });
    let result = (|| {
        send(
            &mut output,
            FrameKind::Hello,
            1,
            json!({"t":"hello","api":yapfast_plugin_api::API_VERSION}),
        )?;
        let ready = wait(&rx, &mut output, "ready", 1)?;
        if ready["api"] != yapfast_plugin_api::API_VERSION {
            bail!("Runtime protocol mismatch")
        }
        send(
            &mut output,
            FrameKind::Request,
            2,
            json!({"t":"activate","plugin":"com.example.ping","load":{"wasm":config.wasm,"memory_pages":128,"wasi":false}}),
        )?;
        wait(&rx, &mut output, "activated", 2)?;
        send(
            &mut output,
            FrameKind::Request,
            3,
            json!({"t":"command","plugin":"com.example.ping","command":{"name":"ping","options":{}}}),
        )?;
        let answer = wait(&rx, &mut output, "command_done", 3)?;
        let text = answer["outcome"]["notice"]["text"]
            .as_str()
            .context("Ping returned an unexpected command response")?
            .to_owned();
        if text != "pong" {
            bail!("The real Ping plugin returned an unexpected result")
        }
        send(
            &mut output,
            FrameKind::Request,
            4,
            json!({"t":"deactivate","plugin":"com.example.ping"}),
        )?;
        wait(&rx, &mut output, "deactivated", 4)?;
        Ok(text)
    })();
    drop(output);
    drop(rx);
    drop(helper);
    let _ = reader.join();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrelated_reply_does_not_satisfy_a_request() {
        let (tx, rx) = mpsc::sync_channel(2);
        tx.send(Ok(Frame::new(
            FrameKind::Hello,
            99,
            br#"{"t":"ready","api":1}"#.to_vec(),
        )))
        .unwrap();
        drop(tx);
        assert!(wait(&rx, &mut Vec::new(), "ready", 1).is_err());
    }

    #[test]
    fn privileged_calls_receive_an_explicit_denial() {
        let (tx, rx) = mpsc::sync_channel(2);
        tx.send(Ok(Frame::new(FrameKind::Request, 7, br#"{"t":"call","plugin":"com.example.ping","call":12,"request":{"method":"messages.send","payload":{}}}"#.to_vec()))).unwrap();
        tx.send(Ok(Frame::new(
            FrameKind::Hello,
            1,
            br#"{"t":"ready","api":1}"#.to_vec(),
        )))
        .unwrap();
        let mut output = Vec::new();
        assert!(wait(&rx, &mut output, "ready", 1).is_ok());
        let response = frame::read_frame(output.as_slice()).unwrap().unwrap();
        assert_eq!(response.kind, FrameKind::Reply);
        assert_eq!(response.id, 7);
        let value: Value = serde_json::from_slice(&response.payload).unwrap();
        assert_eq!(value["call"], 12);
        assert!(value["answer"]["denied"]["reason"].is_string());
    }
}
