use thiserror::Error;

#[derive(Error, Debug)]
pub enum PathError {
    #[error("Immutable paths cannot be modified")]
    Immutable,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("UTF-8 path error: {0}")]
    FromUtf8Error(#[from] std::str::Utf8Error),
}
