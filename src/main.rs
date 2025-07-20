use axum::{
    routing::{get, post},
    Router,
    response::{Html, IntoResponse, Json},
    http::Method,
    Server,
};
use dotenv::dotenv;
use std::{env, net::SocketAddr};
use tower_http::cors::{Any, CorsLayer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod handlers;
mod models;

use crate::handlers::{add_wallet, get_wallet, list_wallets, AppState};
use crate::models::{Wallet, CreateWallet};

/// API documentation
#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::add_wallet,
        handlers::get_wallet,
        handlers::list_wallets,
    ),
    components(schemas(Wallet, CreateWallet, handlers::ErrorResponse)),
    tags(
        (name = "wallets", description = "Wallet management endpoints")
    )
)]
struct ApiDoc;


/// Serve the OpenAPI documentation as HTML
async fn serve_docs() -> impl IntoResponse {
    Html(
        r#"
        <!DOCTYPE html>
        <html>
            <head>
                <title>Degen API Documentation</title>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <style>
                    body { margin: 0; padding: 20px; font-family: Arial, sans-serif; }
                    h1 { color: #333; }
                    .endpoint { margin-bottom: 20px; padding: 15px; background: #f5f5f5; border-radius: 5px; }
                    .method { font-weight: bold; color: #fff; padding: 3px 8px; border-radius: 3px; display: inline-block; margin-right: 10px; }
                    .get { background: #61affe; }
                    .post { background: #49cc90; }
                    .path { font-family: monospace; font-size: 16px; }
                    .description { margin: 10px 0; }
                </style>
            </head>
            <body>
                <h1>Degen API Documentation</h1>
                
                <div class="endpoint">
                    <div><span class="method get">GET</span> <span class="path">/wallets</span></div>
                    <div class="description">List all wallets</div>
                </div>

                <div class="endpoint">
                    <div><span class="method get">GET</span> <span class="path">/wallets/:id</span></div>
                    <div class="description">Get wallet by ID</div>
                </div>

                <div class="endpoint">
                    <div><span class="method post">POST</span> <span class="path">/wallets</span></div>
                    <div class="description">Create a new wallet</div>
                    <div>Example request body: {"address": "0x...", "name": "My Wallet"}</div>
                </div>

                <div style="margin-top: 30px;">
                    <h3>Interactive Documentation</h3>
                    <p>For an interactive API documentation, visit the <a href="/swagger-ui">Swagger UI</a>.</p>
                    <p>Or download the <a href="/openapi.json">OpenAPI specification</a>.</p>
                </div>
            </body>
        </html>
        "#
    )
}

/// Serve the OpenAPI JSON specification
async fn serve_openapi() -> impl IntoResponse {
    Json(ApiDoc::openapi())
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    // Run migrations
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // Enable CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(vec![
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
        ])
        .allow_headers(Any);

    // Create Swagger UI
    let swagger_ui = SwaggerUi::new("/swagger-ui")
        .url("/api-doc/openapi.json", ApiDoc::openapi());

    // Build our application with routes
    let app = Router::new()
        .merge(swagger_ui)
        .route("/docs", get(serve_docs))
        .route("/openapi.json", get(serve_openapi))
        .route("/wallets", post(add_wallet).get(list_wallets))
        .route("/wallets/:id", get(get_wallet))
        .with_state(AppState { db_pool: pool })
        .layer(cors);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Server running at http://{}/docs", addr);
    println!("Swagger UI available at http://{}/swagger-ui", addr);

    Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
