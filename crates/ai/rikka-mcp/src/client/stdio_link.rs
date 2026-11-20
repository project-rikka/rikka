use std::process::Stdio;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::{Mutex, mpsc};

use crate::client::link::{Arrival, LinkError};
use crate::protocol::Message;

#[derive(Debug)]
pub(crate) struct StdioLink {
    input: Arc<Mutex<ChildStdin>>,
    _child: Child,
}

impl StdioLink {
    pub(crate) fn spawn(
        mut command: Command,
        arriving: mpsc::Sender<Arrival>,
    ) -> std::io::Result<(StdioLink, Option<ChildStderr>)> {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::other("the process has no input"))?;
        let output = child
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::other("the process has no output"))?;
        let stderr = child.stderr.take();

        tokio::spawn(read(output, arriving));
        let link = StdioLink {
            input: Arc::new(Mutex::new(input)),
            _child: child,
        };
        Ok((link, stderr))
    }

    pub(crate) async fn send(&self, message: &Message) -> Result<(), LinkError> {
        let mut line = serde_json::to_vec(message).map_err(LinkError::Encode)?;
        line.push(b'\n');

        let input = Arc::clone(&self.input);
        tokio::spawn(async move {
            let mut input = input.lock().await;
            input.write_all(&line).await?;
            input.flush().await
        })
        .await
        .map_err(std::io::Error::other)
        .flatten()
        .map_err(LinkError::Pipe)
    }
}

async fn read(output: ChildStdout, arriving: mpsc::Sender<Arrival>) {
    let mut lines = BufReader::new(output).lines();
    loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => return,
            Err(error) => {
                tracing::debug!("failed to read from an MCP server: {error}");
                return;
            }
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str(line) {
            Ok(message) => {
                if arriving.send(Arrival::Message(message)).await.is_err() {
                    return;
                }
            }
            Err(error) => tracing::debug!("skipping output that is no MCP message: {error}"),
        }
    }
}
