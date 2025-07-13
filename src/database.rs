use sqlx::PgPool;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm,
};
use base64::{Engine as _, engine::general_purpose};
use secrecy::{Secret, ExposeSecret};
use tracing::{info, warn};

// Export aliases for convenience
pub use TokenStore as DbTokenStore;

// =============================================================================
// Database Models
// =============================================================================

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct ShopifyToken {
    pub id: Uuid,
    pub shop_domain: String,
    pub encrypted_access_token: String,
    pub scope: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct OAuthState {
    pub id: Uuid,
    pub state_token: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

// =============================================================================
// Database Configuration
// =============================================================================

#[derive(Clone)]
pub struct DatabaseConfig {
    pub database_url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub encryption_key: Secret<String>,
}

impl DatabaseConfig {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(DatabaseConfig {
            database_url: std::env::var("DATABASE_URL")?,
            max_connections: std::env::var("DB_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()?,
            min_connections: std::env::var("DB_MIN_CONNECTIONS")
                .unwrap_or_else(|_| "5".to_string())
                .parse()?,
            encryption_key: Secret::new(
                std::env::var("ENCRYPTION_KEY")
                    .unwrap_or_else(|_| {
                        warn!("ENCRYPTION_KEY not set, using default (NOT SECURE for production)");
                        "your-32-byte-encryption-key-here-change-this-in-production!".to_string()
                    })
            ),
        })
    }
}

// =============================================================================
// Database Connection Pool
// =============================================================================

pub async fn create_connection_pool(config: &DatabaseConfig) -> Result<PgPool, Box<dyn std::error::Error + Send + Sync>> {
    info!("🔄 Connecting to database...");
    
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .connect(&config.database_url)
        .await?;
    
    info!("✅ Database connection pool created successfully");
    Ok(pool)
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("🔄 Running database migrations...");
    sqlx::migrate!("./migrations").run(pool).await?;
    info!("✅ Database migrations completed");
    Ok(())
}

// =============================================================================
// Token Encryption/Decryption
// =============================================================================

#[derive(Clone)]
pub struct TokenEncryption {
    cipher: Aes256Gcm,
}

impl TokenEncryption {
    pub fn new(key: &Secret<String>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let key_bytes = key.expose_secret().as_bytes();
        if key_bytes.len() != 32 {
            return Err("Encryption key must be exactly 32 bytes".into());
        }
        
        let cipher = Aes256Gcm::new_from_slice(key_bytes)
            .map_err(|e| format!("Failed to create cipher: {}", e))?;
        Ok(Self { cipher })
    }
    
    pub fn encrypt(&self, plaintext: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self.cipher.encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| format!("Encryption failed: {}", e))?;
        
        // Combine nonce + ciphertext and encode as base64
        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ciphertext);
        
        Ok(general_purpose::STANDARD.encode(combined))
    }
    
    pub fn decrypt(&self, encrypted: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let combined = general_purpose::STANDARD.decode(encrypted)?;
        
        if combined.len() < 12 {
            return Err("Invalid encrypted data".into());
        }
        
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = aes_gcm::Nonce::from_slice(nonce_bytes);
        
        let plaintext = self.cipher.decrypt(nonce, ciphertext)
            .map_err(|e| format!("Decryption failed: {}", e))?;
        Ok(String::from_utf8(plaintext)?)
    }
}

// =============================================================================
// Database Operations for Tokens
// =============================================================================

#[derive(Clone)]
pub struct TokenStore {
    pool: PgPool,
    encryption: TokenEncryption,
}

impl TokenStore {
    pub fn new(pool: PgPool, encryption_key: &Secret<String>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let encryption = TokenEncryption::new(encryption_key)?;
        Ok(Self { pool, encryption })
    }
    
    pub async fn store_token(
        &self,
        shop_domain: &str,
        access_token: &str,
        scope: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let encrypted_token = self.encryption.encrypt(access_token)?;
        
        sqlx::query(
            r#"
            INSERT INTO shopify_tokens (shop_domain, encrypted_access_token, scope)
            VALUES ($1, $2, $3)
            ON CONFLICT (shop_domain)
            DO UPDATE SET
                encrypted_access_token = EXCLUDED.encrypted_access_token,
                scope = EXCLUDED.scope,
                updated_at = NOW()
            "#,
        )
        .bind(shop_domain)
        .bind(encrypted_token)
        .bind(scope)
        .execute(&self.pool)
        .await?;
        
        info!("✅ Token stored for shop: {}", shop_domain);
        Ok(())
    }
    
    pub async fn get_token(&self, shop_domain: &str) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let row = sqlx::query_as::<_, (String,)>(
            "SELECT encrypted_access_token FROM shopify_tokens WHERE shop_domain = $1"
        )
        .bind(shop_domain)
        .fetch_optional(&self.pool)
        .await?;
        
        match row {
            Some((encrypted_token,)) => {
                let decrypted_token = self.encryption.decrypt(&encrypted_token)?;
                Ok(Some(decrypted_token))
            }
            None => Ok(None),
        }
    }
    
    pub async fn delete_token(&self, shop_domain: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let result = sqlx::query(
            "DELETE FROM shopify_tokens WHERE shop_domain = $1"
        )
        .bind(shop_domain)
        .execute(&self.pool)
        .await?;
        
        Ok(result.rows_affected() > 0)
    }
    
    pub async fn list_shops(&self) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let rows = sqlx::query_as::<_, (String,)>(
            "SELECT shop_domain FROM shopify_tokens ORDER BY updated_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(rows.into_iter().map(|(shop_domain,)| shop_domain).collect())
    }
}

// =============================================================================
// Database Operations for OAuth States
// =============================================================================

#[derive(Clone)]
pub struct StateStore {
    pool: PgPool,
}

impl StateStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    pub async fn store_state(&self, state_token: &str, ttl_seconds: i64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let expires_at = Utc::now() + chrono::Duration::seconds(ttl_seconds);
        
        sqlx::query(
            r#"
            INSERT INTO oauth_states (state_token, expires_at)
            VALUES ($1, $2)
            "#,
        )
        .bind(state_token)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        
        info!("✅ CSRF state stored: {}", &state_token[..8]);
        Ok(())
    }
    
    pub async fn validate_and_remove_state(&self, state_token: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let result = sqlx::query(
            r#"
            DELETE FROM oauth_states 
            WHERE state_token = $1 AND expires_at > NOW()
            "#,
        )
        .bind(state_token)
        .execute(&self.pool)
        .await?;
        
        let is_valid = result.rows_affected() > 0;
        
        if is_valid {
            info!("✅ CSRF state validated and removed: {}", &state_token[..8]);
        } else {
            warn!("⚠️ CSRF state invalid or expired: {}", &state_token[..8]);
        }
        
        Ok(is_valid)
    }
    
    pub async fn cleanup_expired_states(&self) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let result = sqlx::query_as::<_, (Option<i64>,)>(
            "SELECT cleanup_expired_oauth_states() as deleted_count"
        )
        .fetch_one(&self.pool)
        .await?;
        
        let deleted_count = result.0.unwrap_or(0) as u64;
        
        if deleted_count > 0 {
            info!("🧹 Cleaned up {} expired OAuth states", deleted_count);
        }
        
        Ok(deleted_count)
    }
}
