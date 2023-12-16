use std::io::Error as IoError;
use tokio::sync::watch::error::RecvError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(IoError),
    Send,
    Receive,
    Timeout,
}

impl std::error::Error for Error { }

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TODO: impl Display for Error")
    }
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Self {
        Error::Io(err)
    }
}

impl From<RecvError> for Error {
    fn from(_: RecvError) -> Self {
        Error::Receive
    }
}
