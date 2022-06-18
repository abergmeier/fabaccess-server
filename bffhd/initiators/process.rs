use super::Initiator;
use super::InitiatorCallbacks;
use crate::resources::state::State;
use crate::utils::linebuffer::LineBuffer;
use async_process::{Child, ChildStdout, Command, Stdio};
use futures_lite::{ready, AsyncRead};
use miette::{miette, IntoDiagnostic};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Debug, Serialize, Deserialize)]
pub enum InputMessage {
    #[serde(rename = "state")]
    SetState(State),
}

#[derive(Serialize, Deserialize)]
pub struct OutputLine {}

pub struct Process {
    pub cmd: String,
    pub args: Vec<String>,
    state: Option<ProcessState>,
    buffer: LineBuffer,
    callbacks: InitiatorCallbacks,
}

impl Process {
    fn spawn(&mut self) -> io::Result<()> {
        let mut child = Command::new(&self.cmd)
            .args(&self.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        self.state = Some(ProcessState::new(
            child
                .stdout
                .take()
                .expect("Child just spawned with piped stdout has no stdout"),
            child,
        ));
        Ok(())
    }
}

struct ProcessState {
    pub child: Child,
    pub stdout: ChildStdout,
}

impl ProcessState {
    pub fn new(stdout: ChildStdout, child: Child) -> Self {
        Self { stdout, child }
    }

    fn try_process(&mut self, buffer: &[u8], callbacks: &mut InitiatorCallbacks) -> usize {
        tracing::trace!("trying to process current buffer");

        let mut end = 0;

        while let Some(idx) = buffer[end..].iter().position(|b| *b == b'\n') {
            if idx == 0 {
                end += 1;
                continue;
            }
            let line = &buffer[end..(end + idx)];
            self.process_line(line, callbacks);
            end = idx;
        }

        end
    }

    fn process_line(&mut self, line: &[u8], callbacks: &mut InitiatorCallbacks) {
        match serde_json::from_slice::<InputMessage>(line) {
            Ok(state) => {
                tracing::trace!(?state, "got new state for process initiator");
            }
            Err(error) => tracing::warn!(%error, "process initiator did not send a valid line"),
        }
    }
}

impl Future for Process {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Process {
            state: Some(state),
            buffer,
            callbacks,
            ..
        } = self.get_mut()
        {
            match state.child.try_status() {
                Err(error) => {
                    tracing::error!(%error, "checking child exit code returned an error");
                    return Poll::Ready(());
                }
                Ok(Some(exitcode)) => {
                    tracing::warn!(%exitcode, "child process exited");
                    return Poll::Ready(());
                }
                Ok(None) => {
                    tracing::trace!("process initiator checking on process");

                    let stdout = &mut state.stdout;

                    loop {
                        let buf = buffer.get_mut_write(512);
                        match AsyncRead::poll_read(Pin::new(stdout), cx, buf) {
                            Poll::Pending => break,
                            Poll::Ready(Ok(read)) => {
                                buffer.advance_valid(read);
                                continue;
                            }
                            Poll::Ready(Err(error)) => {
                                tracing::warn!(%error, "reading from child stdout errored");
                                return Poll::Ready(());
                            }
                        }
                    }

                    let processed = state.try_process(buffer, callbacks);
                    buffer.consume(processed);

                    return Poll::Pending;
                }
            }
        } else {
            tracing::warn!("process initiator has no process attached!");
        }

        Poll::Ready(())
    }
}

impl Initiator for Process {
    fn new(params: &HashMap<String, String>, callbacks: InitiatorCallbacks) -> miette::Result<Self>
    where
        Self: Sized,
    {
        let cmd = params
            .get("cmd")
            .ok_or(miette!("Process initiator requires a `cmd` parameter."))?
            .clone();
        let args = params
            .get("args")
            .map(|argv| argv.split_whitespace().map(|s| s.to_string()).collect())
            .unwrap_or_else(Vec::new);
        let mut this = Self {
            cmd,
            args,
            state: None,
            buffer: LineBuffer::new(),
            callbacks,
        };
        this.spawn().into_diagnostic()?;
        Ok(this)
    }
}
