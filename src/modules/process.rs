use std::collections::HashMap;
use std::process::Stdio;
use smol::process::Command;

use futures::future::FutureExt;

use crate::actor::Actuator;
use crate::db::machine::{MachineState, Status};
use futures::future::BoxFuture;

use slog::Logger;

pub struct Process {
    log: Logger,
    name: String,
    cmd: String,
    args: Vec<String>,
}

impl Process {
    pub fn new(log: Logger, name: String, params: &HashMap<String, String>) -> Option<Self> {
        let cmd = params.get("cmd").map(|s| s.to_string())?;
        let args = params.get("args").map(|argv|
                argv.split_whitespace()
                    .map(|s| s.to_string())
                    .collect())
            .unwrap_or_else(Vec::new);

        Some(Self { log, name, cmd, args })
    }

    pub fn into_boxed_actuator(self) -> Box<dyn Actuator + Sync + Send> {
        Box::new(self)
    }
}

impl Actuator for Process {
    fn apply(&mut self, state: MachineState) -> BoxFuture<'static, ()> {
        debug!(self.log, "Running {} ({}) for {:?}", &self.name, &self.cmd, &state);
        let mut command = Command::new(&self.cmd);
        command
            .stdin(Stdio::null())
            .args(self.args.iter())
            .arg(&self.name);

        let fstate = state.state.clone();
        match state.state {
            Status::Free => {
                command.arg("free");
            }
            Status::InUse(by) => {
                command.arg("inuse");
                by.map(|user| command.arg(format!("{}", user)));
            }
            Status::ToCheck(by) => {
                command.arg("tocheck")
                    .arg(format!("{}", by));
            }
            Status::Blocked(by) => {
                command.arg("blocked")
                    .arg(format!("{}", by));
            }
            Status::Disabled => { command.arg("disabled"); },
            Status::Reserved(by) => {
                command.arg("reserved")
                    .arg(format!("{}", by));
            }
        }

        let flog = self.log.new(o!());
        let name = self.name.clone();
        Box::pin(command.output().map(move |res| match res {
            Ok(retv) if retv.status.success() => { 
                trace!(flog, "Actor was successful"); 
                let outstr = String::from_utf8_lossy(&retv.stdout);
                for line in outstr.lines() {
                    debug!(flog, "{}", line);
                }
            }
            Ok(retv) => { 
                warn!(flog, "Actor {} returned nonzero output {} for {:?}", name, retv.status, fstate); 
                if !retv.stderr.is_empty() {
                    let errstr = String::from_utf8_lossy(&retv.stderr);
                    for line in errstr.lines() {
                        warn!(flog, "{}", line);
                    }
                }
            }
            Err(err) => { warn!(flog, "Actor {} failed to run cmd: {}", name, err); }
        }))
    }
}
