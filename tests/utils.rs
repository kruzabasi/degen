use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use degen::{
    handlers::{add_wallet, get_wallet, list_wallets},
    models::{CreateWallet, Wallet},
    AppState,
};
use hyper::body::to_bytes;
use serde::de::DeserializeOwned;

use sqlx::{postgres::PgPoolOptions, PgPool};

use tower::ServiceExt;
use uuid::Uuid;

/// Creates a test application with a fresh database connection
pub async fn create_test_app() -> (Router, PgPool) {
    // Load environment variables
    dotenv::dotenv().ok();

    // In CI, use the provided database URL directly
    if std::env::var("CI").is_ok() {
        let db_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in CI environment");

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&db_url)
            .await
            .expect("Failed to connect to database in CI");

        // Reset the database for the test
        reset_test_database(&pool).await;

        let app = degen::create_app(pool.clone());
        return (app, pool);
    }

    // Local development: Create a new test database
    let db_url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("DATABASE_URL or TEST_DATABASE_URL must be set for tests");

    // Parse the database URL to extract the base URL (without the database name)
    let base_url = db_url
        .rsplitn(2, '/')
        .nth(1)
        .expect("Invalid database URL format");

    // Create a unique test database name
    let test_db_name = format!("test_{}", Uuid::new_v4().to_string().replace("-", ""));
    let test_db_url = format!("{}/{}", base_url, test_db_name);

    // Connect to the postgres database to create the test database
    let root_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to database");

    // Create the test database
    sqlx::query(&format!("CREATE DATABASE {}", test_db_name))
        .execute(&root_pool)
        .await
        .expect("Failed to create test database");

    // Connect to the test database
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&test_db_url)
        .await
        .expect("Failed to connect to test database");

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // Set up cleanup when the test is done
    let root_pool_clone = root_pool.clone();
    let test_db_name_clone = test_db_name.clone();
    tokio::spawn(async move {
        // Wait for the test to complete
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Close all connections to the test database
        sqlx::query("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1")
            .bind(&test_db_name_clone)
            .execute(&root_pool_clone)
            .await
            .ok();

        // Drop the test database
        sqlx::query(&format!("DROP DATABASE IF EXISTS {}", test_db_name_clone))
            .execute(&root_pool_clone)
            .await
            .ok();
    });

    // Create the application with the test database
    let state = AppState {
        db_pool: pool.clone(),
    };
    let app = Router::new()
        .route(
            "/wallets",
            axum::routing::post(add_wallet).get(list_wallets),
        )
        .route("/wallets/:id", axum::routing::get(get_wallet))
        .with_state(state);

    (app, pool)
}

/// Resets the test database to a clean state
#[allow(dead_code)]
pub async fn reset_test_database(pool: &PgPool) {
    // Disable foreign key checks temporarily
    sqlx::query("TRUNCATE TABLE wallets CASCADE")
        .execute(pool)
        .await
        .expect("Failed to clear test data");

    // Reset any sequences to ensure consistent IDs in tests
    sqlx::query("ALTER SEQUENCE wallets_id_seq RESTART WITH 1")
        .execute(pool)
        .await
        .ok(); // This is best effort, not critical if it fails
}

/// Helper function to make test requests and return the raw response
pub async fn make_request_raw<B: serde::Serialize + ?Sized>(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<&B>,
) -> axum::response::Response {
    let request = build_request(method, uri, body);
    app.clone().oneshot(request).await.unwrap()
}

/// Helper function to build a request
fn build_request<B: serde::Serialize + ?Sized>(
    method: &str,
    uri: &str,
    body: Option<&B>,
) -> hyper::Request<hyper::Body> {
    match method {
        "GET" => Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .unwrap(),
        "POST" => {
            let body_bytes = match body {
                Some(b) => Body::from(serde_json::to_vec(b).unwrap()),
                None => Body::empty(),
            };
            Request::builder()
                .method(Method::POST)
                .uri(uri)
                .header("content-type", "application/json")
                .body(body_bytes)
                .unwrap()
        }
        _ => panic!("Unsupported HTTP method: {}", method),
    }
}

/// Helper function to make test requests and deserialize the response
pub async fn make_request<B: serde::Serialize + ?Sized, T: DeserializeOwned>(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<&B>,
) -> (StatusCode, T) {
    let response = make_request_raw(app, method, uri, body).await;
    let status = response.status();
    let body_bytes = to_bytes(response.into_body()).await.unwrap();
    let body: T = serde_json::from_slice(&body_bytes).unwrap_or_else(|e| {
        panic!(
            "Failed to parse response: {}. Body: {}",
            e,
            String::from_utf8_lossy(&body_bytes)
        );
    });

    (status, body)
}

/// Helper function to create a test wallet
pub async fn create_test_wallet(app: &Router, address: &str, name: Option<&str>) -> Wallet {
    // Create a wallet with the given address and name
    let wallet = CreateWallet {
        address: address.to_string(),
        name: name.map(|s| s.to_string()),
    };

    // Make a request to create the wallet
    let (status, wallet): (_, Wallet) = make_request(app, "POST", "/wallets", Some(&wallet)).await;

    // Check that the wallet was created successfully
    assert_eq!(
        status,
        StatusCode::OK,
        "Failed to create test wallet. Status: {}",
        status,
    );

    wallet
}
