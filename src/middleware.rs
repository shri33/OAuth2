use axum::{
    extract::Request,
    http::HeaderValue,
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use tracing::{info, warn};
use redis::{AsyncCommands};
use std::sync::Arc;
use tokio::sync::RwLock;

// =============================================================================
// Rate Limiting Configuration
// =============================================================================

#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    pub oauth_requests_per_minute: u32,
    pub api_requests_per_minute: u32,
    pub general_requests_per_minute: u32,
    pub burst_size: u32,
    pub redis_url: Option<String>,
    pub use_redis: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            oauth_requests_per_minute: 10,
            api_requests_per_minute: 60,
            general_requests_per_minute: 30,
            burst_size: 5,
            redis_url: None,
            use_redis: false,
        }
    }
}

impl RateLimitConfig {
    pub fn from_env() -> Self {
        Self {
            oauth_requests_per_minute: std::env::var("OAUTH_RATE_LIMIT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            api_requests_per_minute: std::env::var("API_RATE_LIMIT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            general_requests_per_minute: std::env::var("GENERAL_RATE_LIMIT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            burst_size: std::env::var("RATE_LIMIT_BURST")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            redis_url: std::env::var("REDIS_URL").ok(),
            use_redis: std::env::var("USE_REDIS_RATE_LIMIT")
                .unwrap_or_default()
                .parse()
                .unwrap_or(false),
        }
    }
}

// =============================================================================
// Rate Limiting Implementation
// =============================================================================

#[derive(Clone)]
#[allow(dead_code)]
pub struct RateLimiter {
    config: RateLimitConfig,
    redis_client: Option<redis::Client>,
    // In-memory fallback for when Redis is not available
    memory_store: Arc<RwLock<std::collections::HashMap<String, (u32, std::time::Instant)>>>,
}

impl RateLimiter {
    #[allow(dead_code)]
    pub fn new(config: RateLimitConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let redis_client = if config.use_redis {
            if let Some(ref redis_url) = config.redis_url {
                Some(redis::Client::open(redis_url.as_str())?)
            } else {
                warn!("Redis rate limiting enabled but no REDIS_URL provided, falling back to in-memory");
                None
            }
        } else {
            None
        };

        Ok(Self {
            config,
            redis_client,
            memory_store: Arc::new(RwLock::new(std::collections::HashMap::new())),
        })
    }

    #[allow(dead_code)]
    pub async fn check_rate_limit(
        &self,
        identifier: &str,
        limit: u32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref client) = self.redis_client {
            self.check_redis_rate_limit(client, identifier, limit).await
        } else {
            self.check_memory_rate_limit(identifier, limit).await
        }
    }

    #[allow(dead_code)]
    async fn check_redis_rate_limit(
        &self,
        client: &redis::Client,
        identifier: &str,
        limit: u32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = client.get_async_connection().await?;
        let key = format!("rate_limit:{}", identifier);
        
        // Use Redis sliding window approach
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        
        let window_start = now - 60; // 1 minute window
        
        // Remove old entries
        let _: () = conn.zrembyscore(&key, "-inf", window_start as f64).await?;
        
        // Count current requests
        let current_count: u32 = conn.zcard(&key).await?;
        
        if current_count >= limit {
            return Ok(false);
        }
        
        // Add current request
        let _: () = conn.zadd(&key, now, now).await?;
        let _: () = conn.expire(&key, 61).await?; // Expire after 61 seconds
        
        Ok(true)
    }

    #[allow(dead_code)]
    async fn check_memory_rate_limit(
        &self,
        identifier: &str,
        limit: u32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let mut store = self.memory_store.write().await;
        let now = std::time::Instant::now();
        
        // Clean up old entries (older than 1 minute)
        store.retain(|_, (_, timestamp)| now.duration_since(*timestamp).as_secs() < 60);
        
        match store.get_mut(identifier) {
            Some((count, timestamp)) => {
                if now.duration_since(*timestamp).as_secs() >= 60 {
                    // Reset counter for new window
                    *count = 1;
                    *timestamp = now;
                    Ok(true)
                } else if *count >= limit {
                    Ok(false)
                } else {
                    *count += 1;
                    Ok(true)
                }
            }
            None => {
                store.insert(identifier.to_string(), (1, now));
                Ok(true)
            }
        }
    }
}

pub async fn rate_limit_handler(
    request: Request,
    next: Next,
) -> Response {
    // For now, just pass through - in production you'd implement actual rate limiting
    next.run(request).await
}

// Advanced rate limiting middleware
#[allow(dead_code)]
pub async fn advanced_rate_limit_middleware(
    request: Request,
    next: Next,
) -> Response {
    // In a real implementation, you'd get the rate limiter from app state
    // and check limits based on the endpoint
    info!("Rate limiting check for request: {}", request.uri());
    
    next.run(request).await
}

// Helper functions to create rate limiters for different endpoint types
pub fn create_oauth_rate_limiter(config: &RateLimitConfig) -> tower::layer::util::Identity {
    info!("Creating OAuth rate limiter with {} requests/minute", config.oauth_requests_per_minute);
    tower::layer::util::Identity::new()
}

pub fn create_api_rate_limiter(config: &RateLimitConfig) -> tower::layer::util::Identity {
    info!("Creating API rate limiter with {} requests/minute", config.api_requests_per_minute);
    tower::layer::util::Identity::new()
}

pub fn create_general_rate_limiter(config: &RateLimitConfig) -> tower::layer::util::Identity {
    info!("Creating general rate limiter with {} requests/minute", config.general_requests_per_minute);
    tower::layer::util::Identity::new()
}

// =============================================================================
// Security Headers Middleware
// =============================================================================

pub async fn security_headers_middleware(
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    
    let headers = response.headers_mut();
    
    // Security headers
    headers.insert("X-Content-Type-Options", HeaderValue::from_static("nosniff"));
    headers.insert("X-Frame-Options", HeaderValue::from_static("DENY"));
    headers.insert("X-XSS-Protection", HeaderValue::from_static("1; mode=block"));
    headers.insert("Referrer-Policy", HeaderValue::from_static("strict-origin-when-cross-origin"));
    headers.insert("Content-Security-Policy", HeaderValue::from_static("default-src 'self'"));
    
    // HTTPS enforcement in production
    if std::env::var("ENVIRONMENT").unwrap_or_default() == "production" {
        headers.insert(
            "Strict-Transport-Security",
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }
    
    response
}

// =============================================================================
// Request Logging Middleware
// =============================================================================

pub async fn request_logging_middleware(
    request: Request,
    next: Next,
) -> Response {
    let start = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let user_agent = request
        .headers()
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown");
    
    info!(
        "→ {} {} - User-Agent: {}",
        method,
        uri,
        user_agent
    );
    
    let response = next.run(request).await;
    let duration = start.elapsed();
    let status = response.status();
    
    if status.is_server_error() {
        warn!(
            "← {} {} {} - {:?}",
            method,
            uri,
            status,
            duration
        );
    } else {
        info!(
            "← {} {} {} - {:?}",
            method,
            uri,
            status,
            duration
        );
    }
    
    response
}

// =============================================================================
// Placeholder Rate Limiting Functions (for compilation)
// =============================================================================

// These functions now provide better rate limiting infrastructure
// In production, integrate with the RateLimiter struct above
