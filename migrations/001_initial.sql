-- Create database schema for Shopify OAuth tokens and state management

-- Table for storing encrypted access tokens
CREATE TABLE shopify_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_domain VARCHAR(255) NOT NULL UNIQUE,
    encrypted_access_token TEXT NOT NULL,
    scope TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    expires_at TIMESTAMPTZ, -- For future token expiration support
    
    INDEX idx_shopify_tokens_shop (shop_domain),
    INDEX idx_shopify_tokens_created (created_at)
);

-- Table for CSRF state management
CREATE TABLE oauth_states (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    state_token VARCHAR(255) NOT NULL UNIQUE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    
    INDEX idx_oauth_states_token (state_token),
    INDEX idx_oauth_states_expires (expires_at)
);

-- Table for rate limiting (optional - can use Redis instead)
CREATE TABLE rate_limit_buckets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    identifier VARCHAR(255) NOT NULL, -- IP address or user ID
    bucket_type VARCHAR(50) NOT NULL, -- 'oauth', 'api', etc.
    token_count INTEGER NOT NULL DEFAULT 0,
    last_refill TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(identifier, bucket_type),
    INDEX idx_rate_limit_identifier (identifier),
    INDEX idx_rate_limit_last_refill (last_refill)
);

-- Function to automatically update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Trigger for shopify_tokens table
CREATE TRIGGER update_shopify_tokens_updated_at
    BEFORE UPDATE ON shopify_tokens
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Function to clean up expired oauth states
CREATE OR REPLACE FUNCTION cleanup_expired_oauth_states()
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM oauth_states WHERE expires_at < NOW();
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ language 'plpgsql';
