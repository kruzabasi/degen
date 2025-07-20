use axum::{
    extract::{Path, Query, State},
    Json,
};
use bs58;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::{CreateWallet, Wallet};
use crate::{AppError, AppState};

/// Helper function to create a conflict error
fn validation_error(message: &str) -> AppError {
    AppError::UnprocessableEntity(message.to_string())
}

fn conflict_error(message: &str) -> AppError {
    AppError::Conflict(message.to_string())
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
        (status = 400, description = "Invalid input"),
        (status = 409, description = "Wallet already exists"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn add_wallet(
    State(state): State<AppState>,
    Json(payload): Json<CreateWallet>,
) -> Result<Json<Wallet>, AppError> {
    info!("Adding new wallet: {:?}", payload);

    // Validate wallet address
    let address = payload.address.trim();
    if address.is_empty() {
        return Err(validation_error("Address cannot be empty"));
    }

    // Validate address length
    if address.len() > 44 {
        return Err(validation_error("Address is too long (max 44 characters)"));
    }

    // Validate base58 encoding
    if bs58::decode(address).into_vec().is_err() {
        return Err(validation_error("Invalid address: must be base58 encoded"));
    }

    // Check for existing wallet with same address
    let exists: bool = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM wallets WHERE address = $1)",
        address
    )
    .fetch_one(&state.db_pool)
    .await?
    .unwrap_or(false);

    if exists {
        warn!("Attempt to add duplicate wallet address: {}", address);
        return Err(conflict_error("Wallet with this address already exists"));
    }

    let id = Uuid::now_v7();
    let now = chrono::Utc::now();

    let wallet = sqlx::query_as::<_, Wallet>(
        r#"
        INSERT INTO wallets (id, address, name, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, address, name, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(address)
    .bind(payload.name)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db_pool)
    .await?;

    info!("Created wallet with ID: {}", id);

    Ok(Json(wallet))
}

/// Get wallet by ID
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
        (status = 404, description = "Wallet not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_wallet(
    Path(wallet_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Wallet>, AppError> {
    info!("Fetching wallet with ID: {}", wallet_id);

    let wallet = sqlx::query_as::<_, Wallet>(
        r#"
        SELECT id, address, name, created_at, updated_at
        FROM wallets
        WHERE id = $1
        "#,
    )
    .bind(wallet_id)
    .fetch_optional(&state.db_pool)
    .await?;

    match wallet {
        Some(wallet) => {
            info!("Found wallet with ID: {wallet_id}");
            Ok(Json(wallet))
        }
        None => {
            warn!("Wallet not found with ID: {wallet_id}");
            Err(AppError::NotFound(format!("Wallet with ID {wallet_id} not found")))
        }
    }
}

/// Pagination parameters for list endpoints
#[derive(Debug, Deserialize, ToSchema)]
pub struct PaginationParams {
    /// Page number (1-based)
    #[serde(default = "default_page")]
    pub page: i64,
    /// Number of items per page (max 100)
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    50
}

/// Paginated response wrapper
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaginatedWallets {
    /// List of wallets in the current page
    pub items: Vec<Wallet>,
    /// Total number of items across all pages
    pub total: i64,
    /// Current page number (1-based)
    pub page: i64,
    /// Number of items per page
    pub per_page: i64,
    /// Total number of pages
    pub total_pages: i64,
}

/// List wallets with pagination
///
/// Returns a paginated list of wallets in the system.
#[utoipa::path(
    get,
    path = "/wallets",
    params(
        ("page" = Option<i64>, Query, description = "Page number (1-based)"),
        ("per_page" = Option<i64>, Query, description = "Number of items per page (max 100)")
    ),
    responses(
        (status = 200, description = "Paginated list of wallets", body = PaginatedWallets),
        (status = 400, description = "Invalid pagination parameters", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
pub async fn list_wallets(
    State(state): State<AppState>,
    pagination: Option<Query<PaginationParams>>,
) -> Result<Json<PaginatedWallets>, AppError> {
    info!("Listing wallets with pagination: {:?}", pagination);

    let pagination = pagination.unwrap_or_else(|| {
        Query(PaginationParams {
            page: default_page(),
            per_page: default_per_page(),
        })
    });

    let page = pagination.page.max(1);
    let per_page = pagination.per_page.clamp(1, 100); // Cap at 100 items per page
    let offset = (page - 1) * per_page;

    // Get total count
    let total_result =
        sqlx::query_scalar::<_, Option<i64>>(r#"SELECT COUNT(*) as count FROM wallets"#)
            .fetch_one(&state.db_pool)
            .await?;

    let total = total_result.unwrap_or(0);

    // Get paginated results
    let wallets = sqlx::query_as!(
        Wallet,
        r#"
        SELECT id, address, name, created_at, updated_at 
        FROM wallets 
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#,
        per_page,
        offset
    )
    .fetch_all(&state.db_pool)
    .await?;

    let total_pages = (total as f64 / per_page as f64).ceil() as i64;

    info!(
        "Returning {} wallets (page {} of {})",
        wallets.len(),
        page,
        total_pages
    );

    Ok(Json(PaginatedWallets {
        items: wallets,
        total,
        page,
        per_page,
        total_pages,
    }))
}
