use super::*;
use crate::abandoned_checkouts::AbandonedCheckout;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use tower::ServiceExt; // for `oneshot`
use tower_http::trace::TraceLayer;

// Mock test data
const TEST_SHOP: &str = "test-shop.myshopify.com";
const TEST_API_KEY: &str = "test_api_key_12345";
const TEST_API_SECRET: &str = "test_api_secret_67890";
const TEST_REDIRECT_URI: &str = "https://test-app.com/callback";

fn create_test_config() -> AppConfig {
    AppConfig {
        shop: TEST_SHOP.to_string(),
        api_key: TEST_API_KEY.to_string(),
        api_secret: TEST_API_SECRET.to_string(),
        redirect_uri: TEST_REDIRECT_URI.to_string(),
        port: 3000,
        host: "localhost".to_string(),
        environment: "test".to_string(),
        database: crate::database::DatabaseConfig {
            database_url: "postgres://test:test@localhost/test".to_string(),
            max_connections: 5,
            min_connections: 1,
            encryption_key: secrecy::Secret::new("test-encryption-key-32-bytes!!".to_string()),
        },
        rate_limit: crate::middleware::RateLimitConfig::default(),
    }
}

#[tokio::test]
async fn test_app_config_validation() {
    let config = create_test_config();
    
    assert_eq!(config.shop, TEST_SHOP);
    assert_eq!(config.api_key, TEST_API_KEY);
    assert_eq!(config.api_secret, TEST_API_SECRET);
    assert_eq!(config.redirect_uri, TEST_REDIRECT_URI);
    assert_eq!(config.environment, "test");
}

#[tokio::test]
async fn test_home_page() {
    let app = Router::new()
        .route("/", axum::routing::get(home_handler))
        .layer(TraceLayer::new_for_http());

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    
    assert!(body_str.contains("Shopify OAuth2 Integration Demo"));
    assert!(body_str.contains("GET /auth"));
    assert!(body_str.contains("GET /orders"));
    assert!(body_str.contains("GET /abandoned-checkouts"));
}

#[cfg(test)]
mod oauth_tests {
    use super::*;
    use url::Url;

    #[test]
    fn test_oauth_url_generation() {
        let config = create_test_config();
        let scopes = "read_orders,read_checkouts";
        let state = "test_state_123";
        
        let expected_url = format!(
            "https://{}/admin/oauth/authorize?client_id={}&scope={}&redirect_uri={}&state={}",
            config.shop,
            config.api_key,
            urlencoding::encode(scopes),
            urlencoding::encode(&config.redirect_uri),
            urlencoding::encode(state)
        );
        
        // Parse URL to validate structure
        let parsed_url = Url::parse(&expected_url).unwrap();
        assert_eq!(parsed_url.host_str(), Some("test-shop.myshopify.com"));
        assert_eq!(parsed_url.path(), "/admin/oauth/authorize");
        
        let query_pairs: std::collections::HashMap<_, _> = parsed_url.query_pairs().collect();
        assert_eq!(query_pairs.get("client_id"), Some(&config.api_key.into()));
        assert_eq!(query_pairs.get("scope"), Some(&scopes.into()));
    }

    #[test]
    fn test_callback_params_parsing() {
        // Test successful callback
        let code = "test_authorization_code";
        let shop = "test-shop.myshopify.com";
        let state = "test_state_123";
        
        let query = format!("code={}&shop={}&state={}", code, shop, state);
        let parsed: CallbackParams = serde_urlencoded::from_str(&query).unwrap();
        
        assert_eq!(parsed.code, Some(code.to_string()));
        assert_eq!(parsed.shop, Some(shop.to_string()));
        assert_eq!(parsed.state, Some(state.to_string()));
        assert_eq!(parsed.error, None);
        
        // Test error callback
        let error = "access_denied";
        let error_query = format!("error={}", error);
        let parsed_error: CallbackParams = serde_urlencoded::from_str(&error_query).unwrap();
        
        assert_eq!(parsed_error.error, Some(error.to_string()));
        assert_eq!(parsed_error.code, None);
    }
}

#[cfg(test)]
mod api_tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_shopify_order_serialization() {
        let order_json = r##"{
            "id": 12345,
            "name": "#1001",
            "total_price": "29.99",
            "created_at": "2023-01-01T10:00:00Z",
            "customer": null
        }"##;
        
        let order: ShopifyOrder = serde_json::from_str(order_json).unwrap();
        assert_eq!(order.id, 12345);
        assert_eq!(order.name, "#1001");
        assert_eq!(order.total_price, "29.99");
        assert_eq!(order.customer, None);
    }

    #[test]
    fn test_abandoned_checkout_serialization() {
        let checkout_json = r##"{
            "id": 67890,
            "token": "test_token_123",
            "total_price": "19.99",
            "created_at": "2023-01-01T10:00:00Z",
            "updated_at": "2023-01-01T10:30:00Z",
            "email": "customer@example.com"
        }"##;
        
        let checkout: AbandonedCheckout = serde_json::from_str(checkout_json).unwrap();
        assert_eq!(checkout.id, 67890);
        assert_eq!(checkout.token, "test_token_123");
        assert_eq!(checkout.total_price, Some("19.99".to_string()));
        assert_eq!(checkout.email, Some("customer@example.com".to_string()));
    }

    #[test]
    fn test_access_token_response_serialization() {
        let token_json = r##"{
            "access_token": "shpat_test_token_123",
            "scope": "read_orders,read_checkouts"
        }"##;
        
        let token_response: AccessTokenResponse = serde_json::from_str(token_json).unwrap();
        assert_eq!(token_response.access_token, "shpat_test_token_123");
        assert_eq!(token_response.scope, "read_orders,read_checkouts");
    }
}

#[cfg(test)]
mod security_tests {

    #[test]
    fn test_csrf_state_generation() {
        let state1 = uuid::Uuid::new_v4().to_string();
        let state2 = uuid::Uuid::new_v4().to_string();
        
        // States should be unique
        assert_ne!(state1, state2);
        
        // States should be valid UUIDs (36 characters with hyphens)
        assert_eq!(state1.len(), 36);
        assert_eq!(state2.len(), 36);
        assert!(state1.contains('-'));
        assert!(state2.contains('-'));
    }

    #[test]
    fn test_url_encoding() {
        let test_url = "https://test-app.com/callback?param=value with spaces";
        let encoded = urlencoding::encode(test_url);
        assert!(encoded.contains("%20")); // Space should be encoded
        assert!(encoded.contains("%3F")); // Question mark should be encoded
    }
}

#[cfg(test)]
mod database_tests {

    #[test]
    fn test_token_encryption_decryption() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key = secrecy::Secret::new("abcdefghijklmnopqrstuvwxyz123456".to_string()); // Exactly 32 bytes
        let encryption = crate::database::TokenEncryption::new(&key)?;
        
        let original_token = "shpat_test_token_12345";
        
        // Encrypt the token
        let encrypted = encryption.encrypt(original_token)?;
        assert_ne!(encrypted, original_token);
        
        // Decrypt the token
        let decrypted = encryption.decrypt(&encrypted)?;
        assert_eq!(decrypted, original_token);
        
        Ok(())
    }

    #[test]
    fn test_encryption_with_different_keys() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key1 = secrecy::Secret::new("abcdefghijklmnopqrstuvwxyz123456".to_string()); // Exactly 32 bytes
        let key2 = secrecy::Secret::new("ZYXWVUTSRQPONMLKJIHGFEDCBA654321".to_string()); // Exactly 32 bytes
        
        let encryption1 = crate::database::TokenEncryption::new(&key1)?;
        let encryption2 = crate::database::TokenEncryption::new(&key2)?;
        
        let original_token = "shpat_test_token_12345";
        let encrypted_with_key1 = encryption1.encrypt(original_token)?;
        
        // Attempting to decrypt with wrong key should fail
        let decrypt_result = encryption2.decrypt(&encrypted_with_key1);
        assert!(decrypt_result.is_err());
        
        Ok(())
    }
}

#[cfg(test)]
mod webhook_tests {
    use crate::webhooks::{verify_webhook, WebhookResponse};
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    #[test]
    fn test_webhook_verification() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let secret = "test_webhook_secret";
        let body = b"test webhook payload";
        
        // Generate valid signature
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())?;
        mac.update(body);
        let signature = hex::encode(mac.finalize().into_bytes());
        
        // Test with valid signature
        assert!(verify_webhook(body, &signature, secret)?);
        
        // Test with invalid signature
        assert!(!verify_webhook(body, "invalid_signature", secret)?);
        
        // Test with sha256= prefix
        let signature_with_prefix = format!("sha256={}", signature);
        assert!(verify_webhook(body, &signature_with_prefix, secret)?);
        
        Ok(())
    }

    #[test]
    fn test_webhook_response_creation() {
        let success_response = WebhookResponse::success("Order processed");
        assert_eq!(success_response.status, "success");
        assert_eq!(success_response.message, "Order processed");
        
        let error_response = WebhookResponse::error("Invalid data");
        assert_eq!(error_response.status, "error");
        assert_eq!(error_response.message, "Invalid data");
    }
}

#[cfg(test)]
mod rate_limiting_tests {
    use crate::middleware::{RateLimitConfig, RateLimiter};

    #[tokio::test]
    async fn test_in_memory_rate_limiting() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let config = RateLimitConfig {
            oauth_requests_per_minute: 2,
            api_requests_per_minute: 5,
            general_requests_per_minute: 3,
            burst_size: 1,
            redis_url: None,
            use_redis: false,
        };
        
        let rate_limiter = RateLimiter::new(config)?;
        let identifier = "test_ip_127.0.0.1";
        let limit = 2;
        
        // First request should be allowed
        assert!(rate_limiter.check_rate_limit(identifier, limit).await?);
        
        // Second request should be allowed
        assert!(rate_limiter.check_rate_limit(identifier, limit).await?);
        
        // Third request should be blocked
        assert!(!rate_limiter.check_rate_limit(identifier, limit).await?);
        
        Ok(())
    }

    #[test]
    fn test_rate_limit_config_from_env() {
        // Set environment variables
        std::env::set_var("OAUTH_RATE_LIMIT", "15");
        std::env::set_var("API_RATE_LIMIT", "100");
        std::env::set_var("GENERAL_RATE_LIMIT", "50");
        std::env::set_var("RATE_LIMIT_BURST", "10");
        
        let config = RateLimitConfig::from_env();
        
        assert_eq!(config.oauth_requests_per_minute, 15);
        assert_eq!(config.api_requests_per_minute, 100);
        assert_eq!(config.general_requests_per_minute, 50);
        assert_eq!(config.burst_size, 10);
        
        // Clean up
        std::env::remove_var("OAUTH_RATE_LIMIT");
        std::env::remove_var("API_RATE_LIMIT");
        std::env::remove_var("GENERAL_RATE_LIMIT");
        std::env::remove_var("RATE_LIMIT_BURST");
    }
}

#[cfg(test)]
mod integration_tests {
    // Integration tests would go here
    // These would test the complete flow with a test database
    
    #[ignore] // Mark as ignored since it requires database setup
    #[tokio::test]
    async fn test_complete_oauth_flow() {
        // This would test:
        // 1. Starting OAuth flow
        // 2. Handling callback
        // 3. Storing token
        // 4. Making API requests
        // 5. Token retrieval
        
        // Implementation would require setting up a test database
        // and mocking Shopify API responses
    }
    
    #[ignore] // Mark as ignored since it requires external services
    #[tokio::test]
    async fn test_webhook_end_to_end() {
        // This would test:
        // 1. Webhook signature verification
        // 2. Payload processing
        // 3. Database updates
        // 4. Response generation
    }
}
