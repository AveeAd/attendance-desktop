use serde::Serializer;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Database(#[from] rusqlite::Error),
    #[error("{0}")]
    InvalidInput(String),
    #[error("Employee not found: {0}")]
    EmployeeNotFound(String),
    #[error("Shift not found: {0}")]
    ShiftNotFound(i64),
    #[error("Event not found: {0}")]
    EventNotFound(i64),
    #[error("Incorrect admin PIN")]
    IncorrectPin,
    #[error("An admin PIN is already configured")]
    PinAlreadySet,
    #[error("No admin PIN is configured")]
    PinNotSet,
    #[error("Failed to parse timestamp: {0}")]
    InvalidTimestamp(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),
    #[error("Excel export error: {0}")]
    Xlsx(#[from] rust_xlsxwriter::XlsxError),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
