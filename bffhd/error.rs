use miette::{Diagnostic, LabeledSpan, Severity, SourceCode};
use std::error;
use std::fmt::{Display, Formatter};
use std::io;
use thiserror::Error;

pub trait Description {
    const DESCRIPTION: Option<&'static str> = None;
    const CODE: &'static str;
    const HELP: Option<&'static str> = None;
    const URL: Option<&'static str> = None;
}

pub fn wrap<D: Description>(error: Source) -> Error {
    Error::new::<D>(error)
}

#[derive(Debug, Error, Diagnostic)]
pub enum Source {
    #[error("io error occured")]
    Io(
        #[source]
        #[from]
        io::Error,
    ),
}

#[derive(Debug)]
pub struct Error {
    description: Option<&'static str>,
    code: &'static str,
    severity: Option<Severity>,
    help: Option<&'static str>,
    url: Option<&'static str>,
    source: Source,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.source, f)
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.source)
    }

    fn description(&self) -> &str {
        if let Some(desc) = self.description {
            desc
        } else {
            self.source.description()
        }
    }
}

impl Error {
    pub fn new<D: Description>(source: Source) -> Self {
        Self {
            description: D::DESCRIPTION,
            code: D::CODE,
            severity: source.severity(),
            help: D::HELP,
            url: D::URL,
            source,
        }
    }
}

impl miette::Diagnostic for Error {
    fn code<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        Some(Box::new(self.code))
    }

    fn severity(&self) -> Option<Severity> {
        self.severity
    }

    fn help<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        self.help.map(|r| {
            let b: Box<dyn Display + 'a> = Box::new(r);
            b
        })
    }

    fn url<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        self.url.map(|r| {
            let b: Box<dyn Display + 'a> = Box::new(r);
            b
        })
    }

    fn diagnostic_source(&self) -> Option<&dyn Diagnostic> {
        Some(&self.source)
    }
}
