use std::fmt;
use std::io;

#[derive(Debug)]
pub enum ReeknoteError {
    Io(io::Error),
    External(String),
    Parse(String),
    InvalidInput(String),
    Unsupported(String),
}

impl fmt::Display for ReeknoteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::External(message) => write!(formatter, "{message}"),
            Self::Parse(message) => write!(formatter, "{message}"),
            Self::InvalidInput(message) => write!(formatter, "{message}"),
            Self::Unsupported(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for ReeknoteError {}

impl From<io::Error> for ReeknoteError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub type Result<T> = std::result::Result<T, ReeknoteError>;
