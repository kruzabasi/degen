mod utils;

extern crate bs58;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use degen::{handlers::PaginatedWallets, models::Wallet};
use dotenv::dotenv as load_dotenv;
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::collections::HashMap;
use std::env;
use std::time::Duration;
use tower::ServiceExt;
use uuid::Uuid;

// Test utilities
use crate::utils::{create_test_app, create_test_wallet, make_request, make_request_raw};

async fn setup_test_db() -> PgPool {
    load_dotenv().ok();

    let database_url = env::var("TEST_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .expect("DATABASE_URL must be set");

    // Create the test database
    let opts: sqlx::postgres::PgConnectOptions =
        database_url.parse().expect("Failed to parse DATABASE_URL");

    // Connect to the postgres database to create the test database
    let root_conn = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(opts.clone())
        .await
        .expect("Failed to connect to PostgreSQL");

    // Create a unique test database name
    let test_db_name = format!("test_{}", Uuid::new_v4().to_string().replace("-", ""));

    // Create the test database
    sqlx::query(&format!("CREATE DATABASE {}", test_db_name))
        .execute(&mut *root_conn.acquire().await.unwrap())
        .await
        .expect("Failed to create test database");

    // Update the database URL to use the test database
    let test_database_url = if let Some(pos) = database_url.rfind('/') {
        format!("{}/{}", &database_url[..pos], test_db_name)
    } else {
        // If there's no '/', just append the test database name
        format!("{}/{}", database_url, test_db_name)
    };

    // Ensure the URL starts with postgres://
    let test_database_url = if !test_database_url.starts_with("postgres://") {
        format!(
            "postgres://{}",
            test_database_url.trim_start_matches("postgresql://")
        )
    } else {
        test_database_url
    };

    // Connect to the test database
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&test_database_url)
        .await
        .expect("Failed to connect to test database");

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // Set up cleanup when the test is done
    let root_conn = root_conn;
    let test_db_name = test_db_name.clone();
    tokio::spawn(async move {
        // Wait for the test to complete
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Close all connections to the test database
        sqlx::query("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1")
            .bind(&test_db_name)
            .execute(&root_conn)
            .await
            .ok();

        // Drop the test database
        sqlx::query(&format!("DROP DATABASE IF EXISTS {}", test_db_name))
            .execute(&root_conn)
            .await
            .ok();
    });

    pool
}

#[tokio::test]
async fn test_wallet_creation() {
    let (app, _pool) = create_test_app().await;
    // Generate a valid base58-encoded wallet address
    let wallet_address = bs58::encode(Uuid::new_v4().as_bytes())
        .into_string()
        .chars()
        .take(44)
        .collect::<String>();

    let (status, _): (_, Value) = make_request::<_, Value>(
        &app,
        "POST",
        "/wallets",
        Some(&json!({
            "address": "",
            "name": "Empty Address"
        })),
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, wallet): (_, Value) = make_request::<_, Value>(
        &app,
        "POST",
        "/wallets",
        Some(&json!({
            "address": wallet_address,
            "name": "Test Wallet"
        })),
    )
    .await;

    let wallet: Wallet = serde_json::from_value(wallet).unwrap();

    assert_eq!(status, StatusCode::OK, "Failed to create wallet");
    assert_eq!(wallet.address, wallet_address);
    assert_eq!(wallet.name, Some("Test Wallet".to_string()));
    assert!(!wallet.id.is_nil());
}

#[tokio::test]
async fn test_duplicate_wallet_address() {
    let (app, _pool) = create_test_app().await;

    // Generate a valid base58-encoded wallet address
    let wallet_address = bs58::encode(Uuid::new_v4().as_bytes())
        .into_string()
        .chars()
        .take(44)
        .collect::<String>();

    // First creation should succeed
    let wallet = create_test_wallet(&app, &wallet_address, None).await;
    assert_eq!(wallet.address, wallet_address);

    // Second creation with same address should fail
    let (status, body): (_, Value) = make_request::<_, Value>(
        &app,
        "POST",
        "/wallets",
        Some(&json!({
            "address": &wallet_address,
            "name": "Duplicate Wallet"
        })),
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT);

    // Verify the error response
    let error = body;
    let error_message = error["error"].as_str().unwrap_or("");
    assert!(
        error_message.ends_with("Wallet with this address already exists"),
        "Expected error message to end with 'Wallet with this address already exists', but got: {}",
        error_message
    );
    assert_eq!(error["code"], "conflict");
}

#[tokio::test]
async fn test_get_wallet() {
    let (app, _pool) = create_test_app().await;
    // Generate a valid base58-encoded wallet address
    let wallet_address = bs58::encode(Uuid::new_v4().as_bytes())
        .into_string()
        .chars()
        .take(44)
        .collect::<String>();

    // Create a wallet first
    let (_, created_wallet): (_, Wallet) = make_request::<_, Wallet>(
        &app,
        "POST",
        "/wallets",
        Some(&json!({
            "address": &wallet_address,
            "name": "Test Wallet"
        })),
    )
    .await;

    // Now get it
    let (status, body): (_, Value) = make_request::<_, Value>(
        &app,
        "GET",
        &format!("/wallets/{}", created_wallet.id),
        None::<&()>,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let retrieved_wallet: Wallet = serde_json::from_value(body).unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(retrieved_wallet.id, created_wallet.id);
    assert_eq!(retrieved_wallet.address, wallet_address);
}

#[tokio::test]
async fn test_list_wallets() {
    let (app, _pool) = create_test_app().await;

    // Create some test wallets with valid base58-encoded addresses
    let wallet1 = create_test_wallet(
        &app,
        &bs58::encode(Uuid::new_v4().as_bytes())
            .into_string()
            .chars()
            .take(44)
            .collect::<String>(),
        Some("Test Wallet 1"),
    )
    .await;
    let wallet2 = create_test_wallet(
        &app,
        &bs58::encode(Uuid::new_v4().as_bytes())
            .into_string()
            .chars()
            .take(44)
            .collect::<String>(),
        Some("Test Wallet 2"),
    )
    .await;

    // List all wallets with default pagination
    let (status, result): (_, PaginatedWallets) =
        make_request::<(), _>(&app, "GET", "/wallets", None::<&()>).await;

    println!("List wallets response: {:?}", result);

    assert_eq!(
        status,
        StatusCode::OK,
        "Expected status code 200, got {}",
        status
    );

    // Debug: Print all wallets in the database
    let wallets: Vec<Wallet> = sqlx::query_as("SELECT * FROM wallets")
        .fetch_all(&_pool)
        .await
        .expect("Failed to query wallets");
    println!("Wallets in database: {:?}", wallets);

    assert_eq!(
        result.items.len(),
        2,
        "Expected 2 wallets, got {}",
        result.items.len()
    );
    assert_eq!(
        result.total, 2,
        "Expected total 2 wallets, got {}",
        result.total
    );
    assert_eq!(result.page, 1, "Expected page 1, got {}", result.page);
    assert_eq!(
        result.per_page, 50,
        "Expected 50 items per page, got {}",
        result.per_page
    );
    assert_eq!(
        result.total_pages, 1,
        "Expected 1 total page, got {}",
        result.total_pages
    );

    // Verify our test wallets are in the list
    let wallet_ids: Vec<Uuid> = result.items.iter().map(|w| w.id).collect();
    println!("Wallet IDs in response: {:?}", wallet_ids);
    println!("Expected wallet1 ID: {}", wallet1.id);
    println!("Expected wallet2 ID: {}", wallet2.id);

    assert!(
        wallet_ids.contains(&wallet1.id),
        "Wallet1 with ID {} not found in response",
        wallet1.id
    );
    assert!(
        wallet_ids.contains(&wallet2.id),
        "Wallet2 with ID {} not found in response",
        wallet2.id
    );

    // Test pagination - first page with 2 items
    let (status, result): (_, PaginatedWallets) = make_request::<_, _>(
        &app,
        "GET",
        &format!("/wallets?page={}&per_page={}", 1, 10),
        None::<&()>,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        result.items.len(),
        2,
        "Should return all 2 wallets on the first page"
    );
    assert_eq!(result.total, 2, "Total should be 2 wallets");
    assert_eq!(result.page, 1, "Should be on page 1");
    assert_eq!(result.per_page, 10, "Per page should be 10");
    assert_eq!(
        result.total_pages, 1,
        "Should only need 1 page for 2 items with per_page=10"
    );

    // Test invalid pagination parameters
    let (status, _): (_, Value) =
        make_request::<_, _>(&app, "GET", "/wallets?page=0&per_page=0", None::<&()>).await;

    assert_eq!(status, StatusCode::OK);

    // Test per_page > 100 (should be capped at 100)
    let (status, result): (_, Value) =
        make_request::<_, _>(&app, "GET", "/wallets?per_page=1000", None::<&()>).await;

    assert_eq!(status, StatusCode::OK);
    let result: HashMap<String, Value> = serde_json::from_value(result).unwrap();
    assert_eq!(result["per_page"], json!(100));
}

/// Test that all migrations can be run successfully and the database schema is correct
/// End-to-end test for wallet operations
/// This test verifies the complete flow of creating, retrieving, and listing wallets
#[tokio::test]
async fn test_wallet_e2e_flow() {
    let (app, _pool) = create_test_app().await;

    // Create a wallet
    // Generate a valid base58-encoded wallet address
    let wallet_address = bs58::encode(Uuid::new_v4().as_bytes())
        .into_string()
        .chars()
        .take(44)
        .collect::<String>();
    let wallet_name = "Test Wallet";

    let (status, wallet): (_, Value) = make_request::<_, _>(
        &app,
        "POST",
        "/wallets",
        Some(&json!({
            "address": wallet_address,
            "name": wallet_name
        })),
    )
    .await;
    let wallet: Wallet = serde_json::from_value(wallet).unwrap();
    assert_eq!(
        status,
        StatusCode::OK,
        "Failed to create wallet. Status: {}",
        status
    );
    assert_eq!(wallet.address, wallet_address);
    assert_eq!(wallet.name, Some(wallet_name.to_string()));
    assert!(!wallet.id.is_nil());

    // Get the created wallet
    let (status, retrieved_wallet): (_, Value) =
        make_request::<_, _>(&app, "GET", &format!("/wallets/{}", wallet.id), None::<&()>).await;
    let retrieved_wallet: Wallet = serde_json::from_value(retrieved_wallet).unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(retrieved_wallet.id, wallet.id);
    assert_eq!(retrieved_wallet.address, wallet_address);
    assert_eq!(retrieved_wallet.name, Some(wallet_name.to_string()));

    // List wallets
    let (status, wallets): (_, Value) =
        make_request::<_, _>(&app, "GET", "/wallets", None::<&()>).await;
    let wallets: PaginatedWallets = serde_json::from_value(wallets).unwrap();

    assert_eq!(status, StatusCode::OK);
    assert!(!wallets.items.is_empty());
    assert!(wallets.items.into_iter().any(|w| w.id == wallet.id));
}

#[tokio::test]
async fn test_migrations() {
    // Set up a test database
    let pool = setup_test_db().await;

    // Verify the schema was created correctly
    let tables = sqlx::query!(
        r#"
        SELECT table_name 
        FROM information_schema.tables 
        WHERE table_schema = 'public'
        "#
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to query tables");

    // Check that all expected tables exist
    let table_names: Vec<Option<String>> = tables.into_iter().map(|r| r.table_name).collect();

    assert!(
        table_names.contains(&Some("wallets".to_string())),
        "wallets table not found"
    );
    assert!(
        table_names.contains(&Some("transactions".to_string())),
        "transactions table not found"
    );

    // Verify the schema of the wallets table
    let wallet_columns = sqlx::query!(
        r#"
        SELECT column_name, data_type, is_nullable
        FROM information_schema.columns
        WHERE table_name = 'wallets'
        "#
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to query wallet columns");

    // Check for required columns in wallets table
    let has_id = wallet_columns
        .iter()
        .any(|c| c.column_name.as_deref() == Some("id"));
    let has_address = wallet_columns
        .iter()
        .any(|c| c.column_name.as_deref() == Some("address"));
    let has_name = wallet_columns
        .iter()
        .any(|c| c.column_name.as_deref() == Some("name"));

    assert!(has_id, "wallets table missing id column");
    assert!(has_address, "wallets table missing address column");
    assert!(has_name, "wallets table missing name column");

    // Verify the schema of the transactions table
    let tx_columns = sqlx::query!(
        r#"
        SELECT column_name, data_type, is_nullable
        FROM information_schema.columns
        WHERE table_name = 'transactions'"#
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to query transaction columns");

    // Check for required columns in transactions table
    let has_wallet_id = tx_columns
        .iter()
        .any(|c| c.column_name.as_deref() == Some("wallet_id"));
    let has_token_address = tx_columns
        .iter()
        .any(|c| c.column_name.as_deref() == Some("token_address"));
    let has_amount = tx_columns
        .iter()
        .any(|c| c.column_name.as_deref() == Some("amount"));

    assert!(has_wallet_id, "transactions table missing wallet_id column");
    assert!(
        has_token_address,
        "transactions table missing token_address column"
    );
    assert!(has_amount, "transactions table missing amount column");
}

#[tokio::test]
async fn test_invalid_wallet_creation() {
    let (app, _pool) = create_test_app().await;

    // Test empty address
    let response = make_request_raw(
        &app,
        "POST",
        "/wallets",
        Some(&json!({ "address": "", "name": "Test Wallet" })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Test missing address
    let response = make_request_raw(
        &app,
        "POST",
        "/wallets",
        Some(&json!({ "name": "Test Wallet" })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Test address that's too long
    let long_address = "x".repeat(100);
    let response = make_request_raw(
        &app,
        "POST",
        "/wallets",
        Some(&json!({
            "address": long_address,
            "name": "Invalid Wallet"
        })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Test invalid base58 address (contains characters not in base58)
    let response = make_request_raw(
        &app,
        "POST",
        "/wallets",
        Some(&json!({
            "address": "0x123",
            "name": "Invalid Wallet"
        })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Test invalid JSON format
    let _response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallets")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{invalid json"))
                .unwrap(),
        )
        .await
        .unwrap();
}
