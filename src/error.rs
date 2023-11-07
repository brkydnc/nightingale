use mavlink;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    MavlinkParserError(mavlink::error::ParserError),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<mavlink::error::ParserError> for Error {
    fn from(err: mavlink::error::ParserError) -> Self {
        Error::MavlinkParserError(err)
    }
}
