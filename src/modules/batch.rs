use std::pin::Pin;
use std::cell::RefCell;

use std::collections::HashMap;
use std::process::Stdio;
use smol::process::{Command, Child};
use smol::io::{AsyncWriteExt, AsyncReadExt};

use futures::future::FutureExt;

use crate::actor::Actuator;
use crate::initiator::Sensor;
use crate::db::machine::{MachineState, Status};
use crate::db::user::{User, Internal as UserDB};
use futures::future::BoxFuture;

use slog::Logger;

use serde::{Serialize, Deserialize};

pub struct Batch {
    userdb: UserDB,
    name: String,
    cmd: String,
    args: Vec<String>,
    kill: bool,
    child: Child,
    stdout: RefCell<Pin<Box<dyn AsyncWrite>>>,
}

impl Batch {
    pub fn new(log: Logger, name: String, params: &HashMap<String, String>, userdb: UserDB)
        -> Option<Self>
    {
        let cmd = params.get("cmd").map(|s| s.to_string())?;
        let args = params.get("args").map(|argv|
                argv.split_whitespace()
                    .map(|s| s.to_string())
                    .collect())
            .unwrap_or_else(Vec::new);

        let kill = params.get("kill_on_exit").and_then(|s|
            s.parse()
             .or_else(|| {
                 warn!(log, "Can't parse `kill_on_exit` for {} set as {} as boolean. \
                             Must be either \"True\" or \"False\".", &name, &s);
                 false
             }));

        info!(log, "Starting {} ({})…", &name, &cmd);
        let mut child = Self::start(&name, &cmd, &args)
            .map_err(|err| error!(log, "Failed to spawn {} ({}): {}", &name, &cmd, err))
            .ok()?;
        let stdout = Self::get_stdin(&mut child);

        Ok(Self { userdb, name, cmd, args, kill, child, stdout })
    }

    fn start_actor(name: &String, cmd: &String, args: &Vec<String>) -> Result<Child> {
        let mut command = Command::new(cmd);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .args(args.iter())
            .arg(name);

        command
            .spawn()
    }

    fn get_stdout(child: &mut Child) -> Pin<Box<dyn AsyncWrite>> {
        let stdout = child.stdout.expect("Actor child has closed stdout");
        stdout.boxed_writer()
    }

    fn maybe_restart(&mut self, f: &mut Option<impl Future<Item=()>>) -> bool {
        let stat = self.child.try_status();
        if stat.is_err() {
            error!(self.log, "Can't check process for {} ({}) [{}]: {}", 
                &self.name, &self.cmd, self.child.id(), stat.unwrap_err());
            return false;
        }
        if let Some(status) = stat.unwrap() {
            warn!(self.log, "Process for {} ({}) exited with code {}", 
                &self.name, &self.cmd, status);
            let errlog = self.log.new(o!("pid" => self.child.id()));
            // If we have any stderr try to log it
            if let Some(stderr) = self.child.stderr.take() {
                f = Some(async move {
                    match stderr.into_stdio().await {
                        Err(err) => error!(errlog, "Failed to open actor process STDERR: ", err),
                        Ok(err) => if !retv.stderr.is_empty() {
                            let errstr = String::from_utf8_lossy(err);
                            for line in errstr.lines() {
                                warn!(errlog, "{}", line);
                            }
                        }
                        _ => {}
                    }
                });
            }
            info!(self.log, "Attempting to re-start {}", &self.name);
            let mut child = Self::start(&self.name, &self.cmd, &self.args)
                .map_err(|err| error!(log, "Failed to spawn {} ({}): {}", &self.name, &self.cmd, err))
                .ok();
            // Nothing else to do with the currect architecture. In reality we should fail here
            // because we *didn't apply* the change.
            if child.is_none() {
                false
            }
            self.child = child.unwrap();
        }

        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StateChangeObj {
    name: String,
    state: MachineState,
}

impl StateChangeObj {
    pub fn new(name: String, state: MachineState) -> Self {
        Self { name, state }
    }
}

impl Actuator for Batch {
    fn apply(&mut self, state: MachineState) -> BoxFuture<'static, ()> {
        debug!(self.log, "Giving {} ({}) new state: {:?}", &self.name, &self.cmd, &state);

        let mut f = None;
        if !self.maybe_restart(&mut f) {
            return Box::pin(futures::future::ready(()));
        }

        let mut json = String::new();
        // Per default compact
        let ser = serde_json::ser::Serializer::new(&mut json);

        let change = StateChangeObj::new(self.name.clone(), state);
        change.serialize(&mut ser);

        // Verify that this "line" does not contain any whitespace.
        debug_assert!(!json.chars().any(|c| c == "\n"));

        let stdin = self.child.stdin.take().expect("Batch actor child has closed stdin?!");

        let errlog = self.log.new(o!("pid" => self.child.id()));
        let g = async move {
            if let Some(f) = f {
                f.await;
            }

            if let Err(e) = stdin.write(json).await {
                error!(errlog, "Failed to send statechange to child: {}", e);
            }
        };

        Box::pin(g);
    }
}
