use thiserror::Error;

#[derive(Error, Debug)]
pub enum RawError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Not a valid Thermo RAW file (OLE2 magic mismatch)")]
    NotRawFile,

    #[error("Unsupported RAW file version: {0}")]
    UnsupportedVersion(u32),

    #[error("Stream not found: {0}")]
    StreamNotFound(String),

    #[error("Scan {0} out of range")]
    ScanOutOfRange(u32),

    #[error("Failed to decode scan data at offset {offset}: {reason}")]
    ScanDecodeError { offset: usize, reason: String },

    #[error("Corrupted data: {0}")]
    CorruptedData(String),

    #[error("OLE2/CFBF error: {0}")]
    CfbError(String),
}
