-- Add migration script here

CREATE TABLE transactions (
    id UUID PRIMARY KEY,
    wallet_id UUID REFERENCES wallets(id),
    token TEXT NOT NULL,
    amount DECIMAL NOT NULL,
    buy_price_usd DECIMAL NOT NULL,
    buy_price_sol DECIMAL NOT NULL,
    timestamp TIMESTAMP NOT NULL
);
