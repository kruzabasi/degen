# Degen - Solana Memecoin Portfolio Tracker

A backend service for tracking Solana memecoin portfolios, built with Rust, Axum, and PostgreSQL.

## Features

- Wallet management (CRUD operations)
- Transaction tracking
- Portfolio analytics
- RESTful API with OpenAPI documentation
- Containerized with Docker
- Comprehensive test suite

## Prerequisites

- Rust (latest stable version)
- PostgreSQL (v12+)
- Docker and Docker Compose (optional, for containerized setup)

## Getting Started

### 1. Clone the repository

```bash
git clone https://github.com/kruzabasi/degen.git
cd degen
```

### 2. Set up environment variables

Create a `.env` file in the project root:

```bash
# Database configuration
DATABASE_URL=postgres://username:password@localhost:5432/degen

# For tests (optional)
TEST_DATABASE_URL=postgres://username:password@localhost:5432/degen_test
```

### 3. Set up the database

#### Option A: Using Docker (recommended)

```bash
# Start PostgreSQL in a container
docker-compose up -d

# Run migrations
sqlx migrate run
```

#### Option B: Manual setup

1. Create a PostgreSQL database:
   ```bash
   createdb degen
   createdb degen_test  # For tests
   ```

2. Run migrations:
   ```bash
   sqlx migrate run
   ```

### 4. Build and run the application

```bash
# Build in release mode
cargo build --release

# Run the application
cargo run --release
```

The API will be available at `http://localhost:3000`

## API Documentation

Once the server is running, you can access:

- **API Documentation**: http://localhost:3000/docs
- **OpenAPI JSON**: http://localhost:3000/openapi.json

## Running Tests

```bash
# Run unit tests
cargo test --lib

# Run integration tests
cargo test --test integration

# Run all tests with logs
RUST_LOG=debug cargo test -- --nocapture
```

## Development

### Code Style

This project uses `rustfmt` for code formatting. Before committing, run:

```bash
cargo fmt
```

### Linting

Run the linter:

```bash
cargo clippy -- -D warnings
```

## Project Structure

```
degen/
├── migrations/       # Database migrations
├── src/             # Source code
│   ├── handlers/    # Request handlers
│   ├── models/      # Data models and database schema
│   ├── lib.rs       # Library entry point
│   └── main.rs      # Application entry point
├── tests/           # Integration tests
└── Cargo.toml       # Project metadata and dependencies
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request
