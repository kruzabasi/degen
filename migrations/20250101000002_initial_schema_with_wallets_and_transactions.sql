-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Create the wallets table with all columns including name
-- Using IF NOT EXISTS to make the migration idempotent
CREATE TABLE IF NOT EXISTS wallets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    address TEXT NOT NULL UNIQUE,
    name TEXT,  -- Optional name for the wallet
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create an index on the address field for faster lookups
CREATE INDEX IF NOT EXISTS wallets_address_idx ON wallets (address);

-- Create the transactions table with IF NOT EXISTS
CREATE TABLE IF NOT EXISTS transactions (
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

-- Create indexes for the transactions table with IF NOT EXISTS
CREATE INDEX IF NOT EXISTS transactions_wallet_id_idx ON transactions (wallet_id);
CREATE INDEX IF NOT EXISTS transactions_token_address_idx ON transactions (token_address);
CREATE INDEX IF NOT EXISTS transactions_transaction_hash_idx ON transactions (transaction_hash);

-- Ensure the name column exists in the wallets table (for backward compatibility)
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                  WHERE table_name = 'wallets' AND column_name = 'name') THEN
        ALTER TABLE wallets ADD COLUMN name TEXT;
    END IF;
END $$;

-- Create a function to update the updated_at column
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create triggers to automatically update the updated_at column
CREATE TRIGGER update_wallets_updated_at
BEFORE UPDATE ON wallets
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_transactions_updated_at
BEFORE UPDATE ON transactions
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

-- Document the schema
COMMENT ON TABLE wallets IS 'Stores wallet information including Solana addresses';
COMMENT ON COLUMN wallets.id IS 'Unique identifier for the wallet';
COMMENT ON COLUMN wallets.address IS 'Wallet address (base58 encoded Solana public key)';
COMMENT ON COLUMN wallets.name IS 'Optional descriptive name for the wallet';
COMMENT ON COLUMN wallets.created_at IS 'Timestamp when the wallet was created';
COMMENT ON COLUMN wallets.updated_at IS 'Timestamp when the wallet was last updated';

COMMENT ON TABLE transactions IS 'Stores token transactions for wallets';
COMMENT ON COLUMN transactions.wallet_id IS 'Reference to the wallet this transaction belongs to';
COMMENT ON COLUMN transactions.token_address IS 'Contract address of the token';
COMMENT ON COLUMN transactions.amount IS 'Amount of tokens transferred';
COMMENT ON COLUMN transactions.buy_price_usd IS 'Price per token in USD at time of transaction';
COMMENT ON COLUMN transactions.buy_price_sol IS 'Price per token in SOL at time of transaction';
