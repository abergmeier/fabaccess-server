use std::collections::HashMap;
use std::process::{Command, Stdio};
use futures_util::future::BoxFuture;
use crate::actors::Actor;
use crate::resources::modules::fabaccess::Status;
use crate::resources::state::State;

pub struct Process {
    name: String,
    cmd: String,
    args: Vec<String>,
}

impl Process {
    pub fn new(name: String, params: &HashMap<String, String>) -> Option<Self> {
        let cmd = params.get("cmd").map(|s| s.to_string())?;
        let args = params.get("args").map(|argv|
            argv.split_whitespace()
                .map(|s| s.to_string())
                .collect())
                         .unwrap_or_else(Vec::new);

        Some(Self { name, cmd, args })
    }

    pub fn into_boxed_actuator(self) -> Box<dyn Actor + Sync + Send> {
        Box::new(self)
    }
}

impl Actor for Process {
    fn apply(&mut self, state: State) -> BoxFuture<'static, ()> {
        tracing::debug!(name=%self.name, cmd=%self.cmd, ?state,
            "Process actor updating state");
        let mut command = Command::new(&self.cmd);
        command
            .stdin(Stdio::null())
            .args(self.args.iter())
            .arg(&self.name);

        match state.inner.state {
            Status::Free => {
                command.arg("free");
            }
            Status::InUse(by) => {
                command.arg("inuse").arg(format!("{}", by.get_username()));
            }
            Status::ToCheck(by) => {
                command.arg("tocheck")
                       .arg(format!("{}", by.get_username()));
            }
            Status::Blocked(by) => {
                command.arg("blocked")
                       .arg(format!("{}", by.get_username()));
            }
            Status::Disabled => { command.arg("disabled"); },
            Status::Reserved(by) => {
                command.arg("reserved")
                       .arg(format!("{}", by.get_username()));
            }
        }

        let name = self.name.clone();
        Box::pin(async move { match command.output() {
            Ok(retv) if retv.status.success() => {
                tracing::trace!("Actor was successful");
                let outstr = String::from_utf8_lossy(&retv.stdout);
                for line in outstr.lines() {
                    tracing::debug!(%name, %line, "actor stdout");
                }
            }
            Ok(retv) => {
                tracing::warn!(%name, ?state, code=?retv.status,
                    "Actor returned nonzero exitcode"
                );
                if !retv.stderr.is_empty() {
                    let errstr = String::from_utf8_lossy(&retv.stderr);
                    for line in errstr.lines() {
                        tracing::warn!(%name, %line, "actor stderr");
                    }
                }
            }
            Err(error) => tracing::warn!(%name, ?error, "process actor failed to run cmd"),
        }})
    }
}
