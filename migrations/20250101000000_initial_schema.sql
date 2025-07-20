-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Create the wallets table
CREATE TABLE wallets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    address TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create an index on the address field for faster lookups
CREATE INDEX wallets_address_idx ON wallets (address);

-- Create the transactions table
CREATE TABLE transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_id UUID NOT NULL REFERENCES wallets(id) ON DELETE CASCADE,
    token_address TEXT NOT NULL,
    token_symbol TEXT NOT NULL,
    name TEXT NOT NULL DEFAULT '',
    amount DECIMAL(78, 18) NOT NULL, -- Supports up to 60 digits before and 18 after decimal
    buy_price_usd DECIMAL(28, 18) NOT NULL,
    buy_price_sol DECIMAL(28, 18) NOT NULL,
    transaction_hash TEXT NOT NULL UNIQUE,
    block_number BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Ensure we have a reference to the wallet
    CONSTRAINT fk_wallet
        FOREIGN KEY(wallet_id) 
        REFERENCES wallets(id)
        ON DELETE CASCADE
);

-- Create indexes for faster lookups
CREATE INDEX transactions_wallet_id_idx ON transactions(wallet_id);
CREATE INDEX transactions_token_address_idx ON transactions(token_address);
CREATE INDEX transactions_block_number_idx ON transactions(block_number);

-- Create a function to update the updated_at column
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create triggers to update the updated_at column
CREATE TRIGGER update_wallets_updated_at
BEFORE UPDATE ON wallets
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_transactions_updated_at
BEFORE UPDATE ON transactions
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();
