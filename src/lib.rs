//! # Degen API
//!
//! A Solana memecoin portfolio tracker API that allows users to track their Solana wallet
//! addresses and view their memecoin portfolios.
//!
//! ## Features
//! - Wallet management (CRUD operations)
//! - Transaction tracking
//! - Portfolio analytics

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use sqlx::{PgPool, Pool};
use std::env;

// Public modules

/// Database models and schemas
pub mod models;

/// Request handlers for API endpoints
pub mod handlers;

/// Custom error types and error handling utilities
pub mod error;

// Re-export commonly used types
pub use crate::error::{
    conflict_error, not_found_error, validation_error, AppError, ErrorResponse,
};
pub use crate::handlers::{add_wallet, get_wallet, list_wallets};
pub use crate::models::{CreateWallet, Wallet};

/// Application state
#[derive(Clone)]
pub struct AppState {
    /// Database connection pool
    pub db_pool: PgPool,
}

/// Establishes a connection to the database using the DATABASE_URL environment variable.
/// # Panics
/// Panics if DATABASE_URL is not set or if the connection fails.
pub async fn establish_connection() -> PgPool {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    Pool::connect(&database_url)
        .await
        .expect("Failed to connect to database")
}

/// Creates a new application state with a database connection pool
pub async fn create_app_state() -> AppState {
    let db_pool = establish_connection().await;
    AppState { db_pool }
}
