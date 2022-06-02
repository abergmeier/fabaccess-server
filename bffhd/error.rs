use thiserror::Error;

use crate::db;
use rsasl::error::SessionError;
use std::any::TypeId;
use std::error::Error as StdError;
use std::fmt;
use std::fmt::Display;
use std::io;

use crate::resources::state::db::StateDBError;
use backtrace::{Backtrace, BacktraceFmt, PrintFmt};
use miette::{Diagnostic, LabeledSpan, Severity, SourceCode};

#[derive(Debug)]
pub struct TracedError<E: Diagnostic> {
    pub inner: E,
    pub backtrace: Backtrace,
}

impl<E: Diagnostic> fmt::Display for TracedError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Error: {}", self.inner)?;

        let cwd = std::env::current_dir();
        let mut print_path =
            move |fmt: &mut fmt::Formatter<'_>, path: backtrace::BytesOrWideString<'_>| {
                let path = path.into_path_buf();
                if let Ok(cwd) = &cwd {
                    if let Ok(suffix) = path.strip_prefix(cwd) {
                        return fmt::Display::fmt(&suffix.display(), fmt);
                    }
                }
                fmt::Display::fmt(&path.display(), fmt)
            };
        let mut bf = BacktraceFmt::new(f, PrintFmt::Short, &mut print_path);
        bf.add_context()?;

        Ok(())
    }
}

impl<E: 'static + Diagnostic> StdError for TracedError<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.inner.source()
    }
}

impl<E: 'static + Diagnostic> Diagnostic for TracedError<E> {
    #[inline(always)]
    fn code<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        self.inner.code()
    }

    #[inline(always)]
    fn severity(&self) -> Option<Severity> {
        self.inner.severity()
    }

    #[inline(always)]
    fn help<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        self.inner.help()
    }

    #[inline(always)]
    fn url<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        self.inner.url()
    }

    #[inline(always)]
    fn source_code(&self) -> Option<&dyn SourceCode> {
        self.inner.source_code()
    }

    #[inline(always)]
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        self.inner.labels()
    }

    #[inline(always)]
    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        self.inner.related()
    }

    #[inline(always)]
    fn diagnostic_source(&self) -> Option<&dyn Diagnostic> {
        self.inner.diagnostic_source()
    }
}

#[derive(Debug, Error, Diagnostic)]
/// Shared error type
pub enum BffhError {
    #[error("SASL error: {0:?}")]
    SASL(SessionError),
    #[error("IO error: {0}")]
    IO(#[from] io::Error),
    #[error("IO error: {0}")]
    Boxed(#[from] Box<dyn std::error::Error>),
    #[error("IO error: {0}")]
    Capnp(#[from] capnp::Error),
    #[error("IO error: {0}")]
    DB(#[from] db::Error),
    #[error("You do not have the permission required to do that.")]
    Denied,
    #[error("State DB operation failed")]
    StateDB(#[from] StateDBError),
}

impl From<SessionError> for BffhError {
    fn from(e: SessionError) -> Self {
        Self::SASL(e)
    }
}

pub type Result<T> = std::result::Result<T, BffhError>;
