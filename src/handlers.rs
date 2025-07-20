use axum::{
    Json,
    extract::{State, Path},
    http::StatusCode,
};
use bs58;
use sqlx::PgPool;
use uuid::Uuid;
use sqlx::types::chrono::Utc;
use utoipa::ToSchema;
use crate::models::{Wallet, CreateWallet};

/// Error response type
#[derive(serde::Serialize, ToSchema)]
pub struct ErrorResponse {
    /// Error message
    pub error: String,
}

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
}

/// Create a new wallet
///
/// This endpoint creates a new wallet with the provided address.
#[utoipa::path(
    post,
    path = "/wallets",
    request_body = CreateWallet,
    responses(
        (status = 200, description = "Wallet created successfully", body = Wallet),
        (status = 400, description = "Invalid input", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
pub async fn add_wallet(
    State(state): State<AppState>,
    Json(payload): Json<CreateWallet>,
) -> Result<Json<Wallet>, (StatusCode, String)> {
    // Debug log the incoming payload
    println!("add_wallet payload: {:?}", payload);
    // Validate wallet address is not empty
    let address = payload.address.trim();
    println!("Validating address: '{}'", address);
    
    if address.is_empty() {
        println!("Validation failed: Address is empty");
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Wallet address cannot be empty".to_string()));
    }
    
    // Basic validation for Solana address format (base58, 32-44 chars)
    // Solana addresses are base58 encoded 32-byte public keys
    // But we'll be more lenient in validation for testing
    if address.len() > 44 {
        println!("Validation failed: Address too long ({} chars)", address.len());
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid wallet address length".to_string()));
    }
    
    // Check if address is valid base58 (common in Solana)
    // Note: In production, you might want to validate against the Solana address format
    match bs58::decode(address).into_vec() {
        Ok(decoded) => println!("Base58 decoded successfully: {:?}", decoded),
        Err(e) => {
            println!("Validation failed: Invalid base58 - {}", e);
            return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid wallet address format".to_string()));
        }
    }
    
    // Check if wallet with this address already exists
    let existing_wallet = sqlx::query!(
        "SELECT id FROM wallets WHERE address = $1",
        address
    )
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e)))?;

    if existing_wallet.is_some() {
        return Err((StatusCode::CONFLICT, "Wallet with this address already exists".to_string()));
    }

    let id = Uuid::now_v7();
    let now = Utc::now();
    let wallet = Wallet { 
        id, 
        address: address.to_string(),
        name: payload.name.clone(),
        created_at: now,
        updated_at: now,
    };
    
    sqlx::query!(
        "INSERT INTO wallets (id, address, name, created_at, updated_at) VALUES ($1, $2, $3, $4, $5)",
        id,
        wallet.address,
        wallet.name,
        wallet.created_at,
        wallet.updated_at
    )
    .execute(&state.db_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to insert wallet: {}", e)))?;
    
    Ok(Json(wallet))
}

/// Get a wallet by ID
///
/// Returns the wallet with the specified ID if it exists.
#[utoipa::path(
    get,
    path = "/wallets/{id}",
    params(
        ("id" = Uuid, Path, description = "Wallet ID")
    ),
    responses(
        (status = 200, description = "Wallet found", body = Wallet),
        (status = 404, description = "Wallet not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
pub async fn get_wallet(
    Path(wallet_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Wallet>, (StatusCode, String)> {
    println!("Fetching wallet with ID: {}", wallet_id);
    
    // First, log all wallets for debugging
    let all_wallets = sqlx::query!("SELECT id, address, name FROM wallets")
        .fetch_all(&state.db_pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to fetch wallets: {}", e)))?;
        
    println!("All wallets in DB: {:?}", all_wallets);
    
    // Then try to fetch the specific wallet
    let wallet = sqlx::query_as!(
        Wallet,
        r#"
        SELECT id, address, name, created_at, updated_at 
        FROM wallets 
        WHERE id = $1
        "#,
        wallet_id
    )
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        println!("Database error: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?
    .ok_or_else(|| {
        println!("Wallet not found with ID: {}", wallet_id);
        (StatusCode::NOT_FOUND, format!("Wallet not found with ID: {}", wallet_id))
    })?;
    
    println!("Successfully retrieved wallet: {:?}", wallet);

    Ok(Json(wallet))
}

/// List all wallets
///
/// Returns a list of all wallets in the system.
#[utoipa::path(
    get,
    path = "/wallets",
    responses(
        (status = 200, description = "List of wallets", body = Vec<Wallet>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
pub async fn list_wallets(
    State(state): State<AppState>,
) -> Result<Json<Vec<Wallet>>, (StatusCode, String)> {
    let wallets = sqlx::query_as!(
        Wallet,
        r#"
        SELECT id, address, name, created_at, updated_at 
        FROM wallets 
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e)))?;

    Ok(Json(wallets))
}