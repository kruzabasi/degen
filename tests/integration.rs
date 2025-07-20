use axum::{
    http::{Request, StatusCode, header},
    routing::{post, get},
    Router,
};
use degen::{add_wallet, get_wallet, list_wallets, models::Wallet};
use dotenv::dotenv;
use hyper::body;
use serde_json::json;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::{env, time::Duration};
use tower::ServiceExt; // for `oneshot`
use uuid::Uuid;
use degen::AppState;

async fn setup_test_db() -> PgPool {
    dotenv().ok();
    
    // Get the database URL from environment
    let database_url = env::var("TEST_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .expect("DATABASE_URL or TEST_DATABASE_URL must be set for tests");
    
    // Connect to the test database
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await
        .expect("Failed to connect to test database");
    
    // Check if migrations have already been run
    let migrations_applied = sqlx::query(
        "SELECT 1 FROM pg_tables WHERE schemaname = 'public' AND tablename = '_sqlx_migrations'"
    )
    .fetch_optional(&pool)
    .await
    .expect("Failed to check for migrations table");
    
    // Run migrations if they haven't been applied yet
    if migrations_applied.is_none() {
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");
    } else {
        // If migrations are already applied, just truncate the tables
        let tables = sqlx::query_scalar::<_, String>(
            "SELECT tablename FROM pg_tables 
             WHERE schemaname = 'public' 
             AND tablename != '_sqlx_migrations'"
        )
        .fetch_all(&pool)
        .await
        .expect("Failed to get table list");
        
        for table in tables {
            sqlx::query(&format!("TRUNCATE TABLE {} CASCADE", table))
                .execute(&pool)
                .await
                .unwrap_or_else(|_| panic!("Failed to truncate table {}", table));
        }
    }
    
    pool
}

async fn create_test_app(pool: PgPool) -> Router {
    let state = AppState { db_pool: pool };
    Router::new()
        .route("/wallets", post(add_wallet).get(list_wallets))
        .route("/wallets/:id", get(get_wallet))
        .with_state(state)
}

async fn create_test_wallet(app: &Router, address: &str, name: Option<&str>) -> Wallet {
    let wallet_data = json!({ 
        "address": address,
        "name": name
    });
    
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallets")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(wallet_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    let body = body::to_bytes(response.into_body()).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn test_wallet_creation() {
    let pool = setup_test_db().await;
    let app = create_test_app(pool).await;

    // Generate unique wallet addresses for this test run
    // Use valid base58 characters for the suffix
    // Valid base58 chars: 123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
    let test_suffix: String = Uuid::now_v7()
        .as_bytes()
        .iter()
        .map(|b| b % 58) // Get a number between 0-57
        .map(|i| match i {
            0..=8 => (b'1' + i) as char,       // 1-9
            9..=16 => (b'A' + (i - 9)) as char, // A-H
            17..=22 => (b'J' + (i - 17)) as char, // J-N
            23..=32 => (b'P' + (i - 23)) as char, // P-Z
            33..=57 => (b'a' + (i - 33)) as char, // a-z (skipping l)
            _ => '1', // Shouldn't happen, but just in case
        })
        .take(3) // Take only 3 chars to keep the total length <= 44
        .collect();
    
    // Ensure the total length is 44 chars or less
    let wallet1_addr = format!("4tqDx5Y5bDiNKWTwyaKdF3qHFDjibZVAwP3n5JtW{}", &test_suffix);
    let wallet2_addr = format!("5KKTqRVf2dXy3Vc8d5q7K3tXvJ9W7Yt8iNn4b3c2v{}", &test_suffix);

    // Test creating a wallet
    let wallet = create_test_wallet(&app, &wallet1_addr, None).await;
    assert_eq!(wallet.address, wallet1_addr);
    
    // Test creating another wallet with a name
    let wallet = create_test_wallet(&app, &wallet2_addr, Some("Test Wallet 2")).await;
    assert_eq!(wallet.address, wallet2_addr);
}

#[tokio::test]
async fn test_duplicate_wallet_address() {
    let pool = setup_test_db().await;
    let app = create_test_app(pool).await;
    
    // Create first wallet
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallets")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    json!({ "address": "7KKTqRVf2dXy3Vc8d5q7K3tXvJ9W7Yt8iNn4b3c2v1a" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    
    // Should return a 409 Conflict for duplicate wallet
    assert_eq!(response.status(), StatusCode::OK);
    
    // Consume the response body
    let _ = body::to_bytes(response.into_body()).await;
    
    // Create second wallet with the same address
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallets")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    json!({ "address": "7KKTqRVf2dXy3Vc8d5q7K3tXvJ9W7Yt8iNn4b3c2v1a" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    
    // Should return a 409 Conflict for duplicate wallet
    assert_eq!(response.status(), StatusCode::CONFLICT);
    
    // Consume the response body
    let _ = body::to_bytes(response.into_body()).await;
}

#[tokio::test]
async fn test_get_wallet() {
    let pool = setup_test_db().await;
    let app = create_test_app(pool).await;
    
    // Create a test wallet with a valid base58 address
    let created_wallet = create_test_wallet(&app, "4tqDx5Y5bDiNKWTwyaKdF3qHFDjibZVAwP3n5JtWjvNz", None).await;
    
    // Test getting the wallet by ID
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/wallets/{}", created_wallet.id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    let body = body::to_bytes(response.into_body()).await.unwrap();
    let wallet: Wallet = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(wallet.id, created_wallet.id);
    assert_eq!(wallet.address, "4tqDx5Y5bDiNKWTwyaKdF3qHFDjibZVAwP3n5JtWjvNz");
    
    // Test getting a non-existent wallet
    let non_existent_id = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/wallets/{}", non_existent_id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_list_wallets() {
    let pool = setup_test_db().await;
    let app = create_test_app(pool).await;
    
    // Create some test wallets with valid base58 addresses
    let wallet1 = create_test_wallet(&app, "8KKTqRVf2dXy3Vc8d5q7K3tXvJ9W7Yt8iNn4b3c2v1a0z9x8y7"[..43].to_string().as_str(), Some("Test Wallet 1")).await;
    let wallet2 = create_test_wallet(&app, "9KKTqRVf2dXy3Vc8d5q7K3tXvJ9W7Yt8iNn4b3c2v1a0z9x8y7"[..43].to_string().as_str(), None).await;
    
    // Test listing all wallets
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/wallets")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    let body = body::to_bytes(response.into_body()).await.unwrap();
    let wallets: Vec<Wallet> = serde_json::from_slice(&body).unwrap();
    
    // We should have at least 2 wallets (there might be more from previous tests)
    assert!(wallets.len() >= 2);
    assert!(wallets.iter().any(|w| w.address == wallet1.address));
    assert!(wallets.iter().any(|w| w.address == wallet2.address));
}

/// Test that all migrations can be run successfully and the database schema is correct
/// End-to-end test for wallet operations
/// This test verifies the complete flow of creating, retrieving, and listing wallets
#[tokio::test]
async fn test_wallet_e2e_flow() {
    // Set up test environment
    let pool = setup_test_db().await;
    let app = create_test_app(pool).await;
    
    // 1. Test creating a new wallet
    let wallet_addr = "4tqDx5Y5bDiNKWTwyaKdF3qHFDjibZVAwP3n5JtWjvN1";
    let wallet_name = "Test Wallet E2E";
    
    
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallets")
                .header("Content-Type", "application/json")
                .body(serde_json::to_vec(&serde_json::json!({
                    "address": wallet_addr,
                    "name": wallet_name
                })).unwrap().into())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(create_response.status(), StatusCode::OK);
    
    // Parse the response to get the created wallet
    let body = hyper::body::to_bytes(create_response.into_body()).await.unwrap();
    let created_wallet: Wallet = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(created_wallet.address, wallet_addr);
    assert_eq!(created_wallet.name, Some(wallet_name.to_string()));
    
    // 2. Test retrieving the created wallet
    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/wallets/{}", created_wallet.id))
                .body(hyper::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(get_response.status(), StatusCode::OK);
    
    let body = hyper::body::to_bytes(get_response.into_body()).await.unwrap();
    let retrieved_wallet: Wallet = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(retrieved_wallet.id, created_wallet.id);
    assert_eq!(retrieved_wallet.address, wallet_addr);
    assert_eq!(retrieved_wallet.name, Some(wallet_name.to_string()));
    
    // 3. Test listing all wallets
    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/wallets")
                .body(hyper::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(list_response.status(), StatusCode::OK);
    
    let body = hyper::body::to_bytes(list_response.into_body()).await.unwrap();
    let wallets: Vec<Wallet> = serde_json::from_slice(&body).unwrap();
    
    // Should contain the wallet we just created
    assert!(wallets.iter().any(|w| w.id == created_wallet.id));
    
    // 4. Test creating a duplicate wallet (should fail)
    let duplicate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallets")
                .header("Content-Type", "application/json")
                .body(serde_json::to_vec(&serde_json::json!({
                    "address": wallet_addr,  // Duplicate address
                    "name": "Duplicate Wallet"
                })).unwrap().into())
                .unwrap(),
        )
        .await
        .unwrap();
    
    // Should return a client error (4xx) for duplicate wallet
    assert!(duplicate_response.status().is_client_error());
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
    let table_names: Vec<Option<String>> = tables.into_iter()
        .map(|r| r.table_name)
        .collect();
    
    assert!(table_names.contains(&Some("wallets".to_string())), "wallets table not found");
    assert!(table_names.contains(&Some("transactions".to_string())), "transactions table not found");
    
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
    let has_id = wallet_columns.iter().any(|c| c.column_name.as_deref() == Some("id"));
    let has_address = wallet_columns.iter().any(|c| c.column_name.as_deref() == Some("address"));
    let has_name = wallet_columns.iter().any(|c| c.column_name.as_deref() == Some("name"));
    
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
    let has_wallet_id = tx_columns.iter().any(|c| c.column_name.as_deref() == Some("wallet_id"));
    let has_token_address = tx_columns.iter().any(|c| c.column_name.as_deref() == Some("token_address"));
    let has_amount = tx_columns.iter().any(|c| c.column_name.as_deref() == Some("amount"));
    
    assert!(has_wallet_id, "transactions table missing wallet_id column");
    assert!(has_token_address, "transactions table missing token_address column");
    assert!(has_amount, "transactions table missing amount column");
}

#[tokio::test]
async fn test_invalid_wallet_creation() {
    let pool = setup_test_db().await;
    let app = create_test_app(pool).await;
    
    // Test creating a wallet with missing address
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallets")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    json!({ "name": "Missing Address" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    
    // Test creating a wallet with empty address
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallets")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    json!({ "address": "" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    
    // Test with invalid JSON
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallets")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    "{invalid json",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}