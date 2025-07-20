use serde::{Deserialize, Serialize};
use sqlx::types::chrono::{DateTime, Utc};
use utoipa::ToSchema;
use uuid::Uuid;

/// Represents a cryptocurrency wallet in the system
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Wallet {
    /// Unique identifier for the wallet
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub id: Uuid,

    /// Blockchain address of the wallet
    #[schema(example = "0x742d35Cc6634C0532925a3b844Bc454e4438f44e")]
    pub address: String,

    /// Optional name for the wallet
    #[schema(example = "My Solana Wallet")]
    pub name: Option<String>,

    /// When the wallet was first added to the system
    #[schema(example = "2025-07-19T17:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When the wallet was last updated
    #[schema(example = "2025-07-19T17:00:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// Request payload for creating a new wallet
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateWallet {
    /// Blockchain address of the wallet
    #[schema(example = "0x742d35Cc6634C0532925a3b844Bc454e4438f44e")]
    pub address: String,

    /// Optional name for the wallet
    #[schema(example = "My Wallet")]
    pub name: Option<String>,
}
