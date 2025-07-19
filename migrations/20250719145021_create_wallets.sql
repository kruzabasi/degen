-- Add migration script here

CREATE TABLE wallets (
    id UUID PRIMARY KEY,
    address TEXT NOT NULL UNIQUE
);
