use thiserror::Error;

/// Top-level error type for the application.
#[derive(Error, Debug)]
pub enum AppError {
    /// Represents I/O errors.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Represents configuration errors.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Represents errors during serialization or deserialization.
    #[error("Serialization/Deserialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Placeholder for network-related errors.
    #[error("Network error: {0}")]
    Network(String),

    /// Placeholder for store-related errors.
    #[error("Store error: {0}")]
    Store(String),

    /// Placeholder for identity-related errors.
    #[error("Identity error: {0}")]
    Identity(String),

    /// Placeholder for access control errors.
    #[error("Access control error: {0}")]
    Access(String),

    /// Placeholder for service adaptation layer errors.
    #[error("Service error: {0}")]
    Service(String),

    /// Placeholder for audit log errors.
    #[error("Audit error: {0}")]
    Audit(String),
}

// Define a standard result type for the application
pub type AppResult<T> = Result<T, AppError>;
