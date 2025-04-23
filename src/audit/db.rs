use crate::errors::AppError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool};

/// Represents a single audit log entry.
#[derive(Serialize, Deserialize, Debug, Clone)] // Added Clone
pub struct AuditEvent {
    // Using String for simplicity, could be an enum later
    pub event_type: String,
    // Optional DID of the actor performing the action
    pub actor_did: Option<String>,
    // Optional Node ID of the actor
    pub actor_node_id: Option<String>,
    // Target identifier (e.g., Transaction ID, Peer Node ID)
    pub target: Option<String>,
    // Timestamp of the event
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    // Additional details specific to the event type
    pub details: Option<serde_json::Value>,
}

/// Service responsible for connecting to and logging audit events to the PostgreSQL database.
#[derive(Clone)] // Pool is Clone
pub struct AuditLogger {
    pool: PgPool,
}

impl AuditLogger {
    /// Creates a new AuditLogger and establishes a connection pool.
    ///
    /// Reads the database connection URL from the `AUDIT_DATABASE_URL` environment variable.
    pub async fn new() -> Result<Self, AppError> {
        let database_url = std::env::var("AUDIT_DATABASE_URL")
            .map_err(|e| AppError::Config(format!("Missing AUDIT_DATABASE_URL: {}", e)))?;

        let pool = PgPoolOptions::new()
            .max_connections(5) // Example: configure pool size
            .connect(&database_url)
            .await
            .map_err(|e| AppError::Audit(format!("Failed to connect to audit DB: {}", e)))?;

        // TODO: Run database migrations here if necessary

        Ok(AuditLogger { pool })
    }

    /// Logs an audit event to the database.
    ///
    /// (Currently a placeholder - does not perform DB insertion yet)
    pub async fn log_event(&self, event: AuditEvent) -> Result<(), AppError> {
        // Placeholder: In a real implementation, this would insert the event into the audit_log table.
        // Example (requires defining the table and handling potential errors):
        /*
        sqlx::query!(
            r#"
            INSERT INTO audit_log (event_type, actor_did, actor_node_id, target, timestamp, details)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            event.event_type,
            event.actor_did,
            event.actor_node_id,
            event.target,
            event.timestamp,
            event.details
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Audit(format!("Failed to log audit event: {}", e)))?;
        */

        // Use tracing to log the event for now
        tracing::info!(target: "audit", event = ?event, "Placeholder: Audit event received");

        Ok(())
    }

    /// Returns a reference to the connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
