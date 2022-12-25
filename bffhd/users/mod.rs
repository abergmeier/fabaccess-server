use std::fs;

use lmdb::{Environment, Transaction};
use once_cell::sync::OnceCell;
use rkyv::{Archive, Deserialize, Infallible, Serialize};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::io::Write;

use clap::ArgMatches;
use miette::{Context, Diagnostic, IntoDiagnostic, SourceOffset, SourceSpan};
use std::path::Path;
use std::sync::Arc;

use thiserror::Error;

pub mod db;

use crate::users::db::UserData;
use crate::UserDB;

#[derive(
    Clone,
    PartialEq,
    Eq,
    Debug,
    Archive,
    Serialize,
    Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
#[archive_attr(derive(Debug, PartialEq))]
pub struct UserRef {
    pub id: String,
}

impl PartialEq<ArchivedUserRef> for UserRef {
    fn eq(&self, other: &ArchivedUserRef) -> bool {
        self.id == other.id
    }
}
impl PartialEq<UserRef> for ArchivedUserRef {
    fn eq(&self, other: &UserRef) -> bool {
        self.id == other.id
    }
}

impl Display for ArchivedUserRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id.as_str())
    }
}

impl UserRef {
    pub fn new(id: String) -> Self {
        UserRef { id }
    }

    pub fn get_username(&self) -> &str {
        self.id.as_str()
    }
}

static USERDB: OnceCell<UserDB> = OnceCell::new();

#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct Users {
    userdb: &'static UserDB,
}

#[derive(Clone, Debug, PartialEq, Eq, Error, Diagnostic)]
#[error(transparent)]
#[repr(transparent)]
pub struct Error(#[from] pub db::Error);

impl Users {
    pub fn new(env: Arc<Environment>) -> Result<Self, Error> {
        let span = tracing::debug_span!("users", ?env, "Creating Users handle");
        let _guard = span.enter();

        let userdb = USERDB.get_or_try_init(|| {
            tracing::debug!("Global resource not yet initialized, initializing…");
            unsafe { UserDB::create(env) }
        })?;

        Ok(Self { userdb })
    }

    pub(crate) fn into_inner(self) -> &'static UserDB {
        self.userdb
    }

    pub fn get_user(&self, uid: &str) -> Option<db::User> {
        tracing::trace!(uid, "Looking up user");
        self.userdb.get(uid).unwrap().map(|user| {
            Deserialize::<db::User, _>::deserialize(user.as_ref(), &mut Infallible).unwrap()
        })
    }

    pub fn put_user(&self, uid: &str, user: &db::User) -> Result<(), crate::db::Error> {
        tracing::trace!(uid, ?user, "Updating user");
        self.userdb.put(uid, user)
    }

    pub fn del_user(&self, uid: &str) -> Result<(), crate::db::Error> {
        tracing::trace!(uid, "Deleting user");
        self.userdb.delete(uid)
    }

    pub fn load_file(&self, path_str: &str) -> miette::Result<()> {
        let path: &Path = Path::new(path_str);
        if path.is_dir() {
            #[derive(Debug, Error, Diagnostic)]
            #[error("load takes a file, not a directory")]
            #[diagnostic(
                code(load::file),
                url("https://gitlab.com/fabinfra/fabaccess/bffh/-/issues/55")
            )]
            struct LoadIsDirError {
                #[source_code]
                src: String,

                #[label("path provided")]
                dir_path: SourceSpan,

                #[help]
                help: String,
            }

            Err(LoadIsDirError {
                src: format!("--load {}", path_str),
                dir_path: (7, path_str.as_bytes().len()).into(),
                help: format!(
                    "Provide a path to a file instead, e.g. {}/users.toml",
                    path_str
                ),
            })?;
            return Ok(());
        }
        let f = std::fs::read(path).into_diagnostic()?;
        let map: HashMap<String, UserData> = toml::from_slice(&f).into_diagnostic()?;

        let mut txn = unsafe { self.userdb.get_rw_txn()? };

        self.userdb.clear_txn(&mut txn)?;

        for (uid, mut userdata) in map {
            userdata.passwd = userdata.passwd.map(|pw| {
                if !pw.starts_with("$argon2") {
                    let config = argon2::Config::default();
                    let salt: [u8; 16] = rand::random();
                    let hash = argon2::hash_encoded(pw.as_bytes(), &salt, &config)
                        .expect(&format!("Failed to hash password for {}: ", uid));
                    tracing::debug!("Hashed pw for {} to {}", uid, hash);

                    hash
                } else {
                    pw
                }
            });
            let user = db::User {
                id: uid.clone(),
                userdata,
            };
            tracing::trace!(%uid, ?user, "Storing user object");
            if let Err(e) = self.userdb.put_txn(&mut txn, uid.as_str(), &user) {
                tracing::warn!(error=?e, "failed to add user")
            }
        }

        txn.commit().map_err(crate::db::Error::from)?;
        Ok(())
    }

    pub fn dump_file(&self, path_str: &str, force: bool) -> miette::Result<usize> {
        let path = Path::new(path_str);
        let exists = path.exists();
        if exists {
            if !force {
                #[derive(Debug, Error, Diagnostic)]
                #[error("given file already exists, refusing to clobber")]
                #[diagnostic(code(dump::clobber))]
                struct DumpFileExists {
                    #[source_code]
                    src: String,

                    #[label("file provided")]
                    dir_path: SourceSpan,

                    #[help]
                    help: &'static str,
                }

                Err(DumpFileExists {
                    src: format!("--load {}", path_str),
                    dir_path: (7, path_str.as_bytes().len()).into(),
                    help: "to force overwriting the file add `--force` as argument",
                })?;
            } else {
                tracing::info!("output file already exists, overwriting due to `--force`");
            }
        }
        let mut file = fs::File::create(path).into_diagnostic()?;

        let users = self.userdb.get_all()?;
        let encoded = toml::ser::to_vec(&users).into_diagnostic()?;
        file.write_all(&encoded[..]).into_diagnostic()?;

        Ok(0)
    }
}
