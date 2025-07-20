use sqlx::{PgPool, Pool};
use std::env;

// Public modules
pub mod models;
pub mod handlers;

// Re-export commonly used types
pub use handlers::{AppState, add_wallet, get_wallet, list_wallets};
pub use models::{Wallet, CreateWallet};

/// Establishes a connection to the database using the DATABASE_URL environment variable.
/// # Panics
/// Panics if DATABASE_URL is not set or if the connection fails.
pub async fn establish_connection() -> PgPool {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    Pool::connect(&database_url).await.expect("Failed to connect to database")
}