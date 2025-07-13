use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::get,
    Json, Router,
    middleware as axum_middleware,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;
use tracing::{info, warn, error};

mod database;
mod middleware;
mod http_client;
mod shopify_api;
mod webhooks;
mod abandoned_checkouts;

#[cfg(test)]
mod tests;

use database::{
    create_connection_pool, run_migrations, DatabaseConfig,
    TokenStore as DbTokenStore, StateStore as DbStateStore,
};
use middleware::{
    RateLimitConfig, create_oauth_rate_limiter, create_api_rate_limiter, 
    create_general_rate_limiter, security_headers_middleware, 
    request_logging_middleware, rate_limit_handler,
};
use shopify_api::{products_handler, customers_handler, inventory_handler};
use abandoned_checkouts::{abandoned_checkouts_handler, abandoned_checkouts_count_handler};
use webhooks::{
    orders_created_webhook, orders_updated_webhook, orders_cancelled_webhook,
    products_created_webhook, customers_created_webhook, 
    checkouts_created_webhook, checkouts_updated_webhook, list_webhooks_handler,
};

// =============================================================================
// Configuration and Types
// =============================================================================

#[derive(Clone)]
pub struct AppConfig {
    pub shop: String,
    pub api_key: String,
    pub api_secret: String,
    pub redirect_uri: String,
    pub port: u16,
    pub host: String,
    pub environment: String,
    pub database: DatabaseConfig,
    pub rate_limit: RateLimitConfig,
}

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub token_store: DbTokenStore,
    pub state_store: DbStateStore,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        dotenv::dotenv().ok();
        
        Ok(AppConfig {
            shop: std::env::var("SHOP")?,
            api_key: std::env::var("API_KEY")?,
            api_secret: std::env::var("API_SECRET")?,
            redirect_uri: std::env::var("REDIRECT_URI")?,
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()?,
            host: std::env::var("HOST")
                .unwrap_or_else(|_| "0.0.0.0".to_string()),
            environment: std::env::var("ENVIRONMENT")
                .unwrap_or_else(|_| "development".to_string()),
            database: DatabaseConfig::from_env()?,
            rate_limit: RateLimitConfig::from_env(),
        })
    }
}

// OAuth2 callback parameters
#[derive(Deserialize)]
pub struct CallbackParams {
    pub code: Option<String>,
    pub shop: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

// Shopify access token response
#[derive(Deserialize, Serialize)]
pub struct AccessTokenResponse {
    pub access_token: String,
    pub scope: String,
}

// Shopify Order structure (simplified)
#[derive(Deserialize, Serialize)]
pub struct ShopifyOrder {
    pub id: u64,
    pub name: String,
    pub total_price: String,
    pub created_at: String,
    pub customer: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize)]
pub struct OrdersResponse {
    pub orders: Vec<ShopifyOrder>,
}

// =============================================================================
// Helper Functions
// =============================================================================

pub async fn get_token(token_store: &DbTokenStore, shop: &str) -> Option<String> {
    match token_store.get_token(shop).await {
        Ok(Some(token)) => Some(token),
        Ok(None) => {
            warn!("No access token found for shop: {}", shop);
            None
        }
        Err(e) => {
            error!("Database error retrieving token for shop {}: {}", shop, e);
            None
        }
    }
}

// =============================================================================
// OAuth2 Flow Implementation
// =============================================================================

pub async fn auth_handler(State(state): State<AppState>) -> impl IntoResponse {
    let scopes = "read_orders,read_checkouts"; // Add more scopes as needed
    let csrf_state = uuid::Uuid::new_v4().to_string(); // CSRF protection
    
    // Store CSRF state for validation (10 minutes TTL)
    if let Err(e) = state.state_store.store_state(&csrf_state, 600).await {
        error!("Failed to store CSRF state: {}", e);
        return Html(
            r#"<h1>❌ Internal Error</h1>
            <p>Unable to initiate OAuth flow. Please try again.</p>
            <a href="/">← Back to Home</a>"#.to_string()
        ).into_response();
    }
    
    let auth_url = format!(
        "https://{}/admin/oauth/authorize?client_id={}&scope={}&redirect_uri={}&state={}",
        state.config.shop,
        state.config.api_key,
        urlencoding::encode(scopes),
        urlencoding::encode(&state.config.redirect_uri),
        urlencoding::encode(&csrf_state)
    );
    
    info!("Redirecting to Shopify OAuth: {}", auth_url);
    Redirect::permanent(&auth_url).into_response()
}

pub async fn oauth_callback(
    Query(params): Query<CallbackParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Handle OAuth errors
    if let Some(error) = params.error {
        error!("OAuth error: {}", error);
        return Html(format!(
            r#"<h1>❌ OAuth Error</h1>
            <p>Error: {}</p>
            <a href="/">← Back to Home</a>"#, 
            error
        ));
    }
    
    // Validate required parameters
    let code = match params.code {
        Some(code) => code,
        None => {
            error!("Missing authorization code");
            return Html(
                r#"<h1>❌ Error</h1>
                <p>Missing authorization code</p>
                <a href="/auth">Try OAuth again</a>"#.to_string()
            );
        }
    };
    
    let shop = params.shop.unwrap_or_else(|| state.config.shop.clone());
    
    // Validate CSRF state parameter for security
    if let Some(ref received_state) = params.state {
        match state.state_store.validate_and_remove_state(received_state).await {
            Ok(true) => {
                info!("✅ CSRF state validation passed");
            }
            Ok(false) => {
                error!("CSRF state validation failed for state: {}", &received_state[..8]);
                return Html(
                    r#"<!DOCTYPE html>
                    <html>
                    <head>
                        <title>Security Error</title>
                        <style>
                            body { font-family: Arial, sans-serif; max-width: 600px; margin: 50px auto; padding: 20px; text-align: center; }
                            .error { color: #dc3545; }
                            .button { background: #007bff; color: white; padding: 10px 20px; text-decoration: none; border-radius: 5px; display: inline-block; margin: 10px; }
                        </style>
                    </head>
                    <body>
                        <h1 class="error">🚨 Security Error</h1>
                        <p>Invalid or expired security token. This could indicate a potential security issue.</p>
                        <p>Please try the OAuth flow again.</p>
                        <a href="/auth" class="button">🔄 Start OAuth Again</a>
                        <a href="/">← Back to Home</a>
                    </body>
                    </html>"#.to_string()
                );
            }
            Err(e) => {
                error!("Database error during CSRF validation: {}", e);
                return Html(
                    r#"<!DOCTYPE html>
                    <html>
                    <head>
                        <title>System Error</title>
                        <style>
                            body { font-family: Arial, sans-serif; max-width: 600px; margin: 50px auto; padding: 20px; text-align: center; }
                            .error { color: #dc3545; }
                            .button { background: #007bff; color: white; padding: 10px 20px; text-decoration: none; border-radius: 5px; display: inline-block; margin: 10px; }
                        </style>
                    </head>
                    <body>
                        <h1 class="error">🚨 System Error</h1>
                        <p>Unable to validate security token. Please try again.</p>
                        <a href="/auth" class="button">🔄 Start OAuth Again</a>
                        <a href="/">← Back to Home</a>
                    </body>
                    </html>"#.to_string()
                );
            }
        }
    } else {
        warn!("⚠️ No CSRF state received in callback");
        return Html(
            r#"<!DOCTYPE html>
            <html>
            <head>
                <title>Security Error</title>
                <style>
                    body { font-family: Arial, sans-serif; max-width: 600px; margin: 50px auto; padding: 20px; text-align: center; }
                    .error { color: #dc3545; }
                    .button { background: #007bff; color: white; padding: 10px 20px; text-decoration: none; border-radius: 5px; display: inline-block; margin: 10px; }
                </style>
            </head>
            <body>
                <h1 class="error">🚨 Security Error</h1>
                <p>Missing security token. This could indicate a potential security issue.</p>
                <p>Please try the OAuth flow again.</p>
                <a href="/auth" class="button">🔄 Start OAuth Again</a>
                <a href="/">← Back to Home</a>
            </body>
            </html>"#.to_string()
        );
    }
    
    info!("✅ OAuth callback received for shop: {} with code: {}", shop, &code[..8]);
    
    // Exchange authorization code for access token
    match exchange_code_for_token(&code, &shop, &state.config).await {
        Ok(token_response) => {
            info!("✅ Successfully exchanged code for access token");
            
            // Store the access token
            if let Err(e) = state.token_store.store_token(&shop, &token_response.access_token, &token_response.scope).await {
                error!("Failed to store access token: {}", e);
                return Html(format!(
                    r#"<!DOCTYPE html>
                    <html>
                    <head>
                        <title>Storage Error</title>
                        <style>
                            body {{ font-family: Arial, sans-serif; max-width: 600px; margin: 50px auto; padding: 20px; text-align: center; }}
                            .error {{ color: #dc3545; }}
                            .button {{ background: #007bff; color: white; padding: 10px 20px; text-decoration: none; border-radius: 5px; display: inline-block; margin: 10px; }}
                        </style>
                    </head>
                    <body>
                        <h1 class="error">❌ Storage Error</h1>
                        <p>OAuth was successful but failed to store the access token.</p>
                        <p><strong>Error:</strong> {}</p>
                        <br>
                        <a href="/auth" class="button">🔄 Try OAuth Again</a>
                        <a href="/">← Back to Home</a>
                    </body>
                    </html>"#,
                    e
                ));
            }
            
            Html(format!(
                r#"<!DOCTYPE html>
                <html>
                <head>
                    <title>OAuth Success</title>
                    <style>
                        body {{ font-family: Arial, sans-serif; max-width: 600px; margin: 50px auto; padding: 20px; text-align: center; }}
                        .success {{ color: #28a745; }}
                        .button {{ background: #007bff; color: white; padding: 10px 20px; text-decoration: none; border-radius: 5px; display: inline-block; margin: 10px; }}
                        .token-info {{ background: #f8f9fa; padding: 15px; border-radius: 5px; margin: 20px 0; }}
                    </style>
                </head>
                <body>
                    <h1 class="success">✅ OAuth Success!</h1>
                    <p>Successfully connected to shop: <strong>{}</strong></p>
                    <div class="token-info">
                        <h3>🔑 Token Information</h3>
                        <p><strong>Access Token:</strong> {}...</p>
                        <p><strong>Granted Scopes:</strong> {}</p>
                    </div>
                    <h3>🎉 Ready to use the API!</h3>
                    <a href="/orders" class="button">📦 View Orders</a>
                    <a href="/abandoned-checkouts" class="button">🛒 Abandoned Checkouts</a>
                    <br><br>
                    <a href="/">← Back to Home</a>
                </body>
                </html>"#,
                shop, 
                &token_response.access_token[..12],
                token_response.scope
            ))
        }
        Err(e) => {
            error!("Failed to exchange code for token: {}", e);
            Html(format!(
                r#"<!DOCTYPE html>
                <html>
                <head>
                    <title>OAuth Error</title>
                    <style>
                        body {{ font-family: Arial, sans-serif; max-width: 600px; margin: 50px auto; padding: 20px; text-align: center; }}
                        .error {{ color: #dc3545; }}
                        .button {{ background: #007bff; color: white; padding: 10px 20px; text-decoration: none; border-radius: 5px; display: inline-block; margin: 10px; }}
                    </style>
                </head>
                <body>
                    <h1 class="error">❌ Token Exchange Failed</h1>
                    <p>Failed to exchange authorization code for access token.</p>
                    <p><strong>Error:</strong> {}</p>
                    <br>
                    <a href="/auth" class="button">🔄 Try OAuth Again</a>
                    <a href="/">← Back to Home</a>
                </body>
                </html>"#,
                e
            ))
        }
    }
}

// =============================================================================
// Token Exchange Implementation
// =============================================================================

async fn exchange_code_for_token(
    code: &str,
    shop: &str,
    config: &AppConfig,
) -> Result<AccessTokenResponse, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    
    // Prepare token exchange request
    let token_url = format!("https://{}/admin/oauth/access_token", shop);
    
    let token_request = serde_json::json!({
        "client_id": config.api_key,
        "client_secret": config.api_secret,
        "code": code
    });
    
    info!("🔄 Exchanging authorization code for access token...");
    info!("Token URL: {}", token_url);
    
    let response = client
        .post(&token_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&token_request)
        .send()
        .await?;
    
    let status = response.status();
    
    if !status.is_success() {
        let error_text = response.text().await?;
        error!("Token exchange failed with status {}: {}", status, error_text);
        return Err(format!("Shopify token exchange failed: {} - {}", status, error_text).into());
    }
    
    let token_response: AccessTokenResponse = response.json().await?;
    
    info!("✅ Token exchange successful! Granted scopes: {}", token_response.scope);
    
    Ok(token_response)
}

// =============================================================================
// Shopify API Endpoints
// =============================================================================

pub async fn orders_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let shop = &state.config.shop;
    
    // Get stored access token
    let token = match state.token_store.get_token(shop).await {
        Ok(Some(token)) => token,
        Ok(None) => {
            warn!("No access token found for shop: {}", shop);
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "No access token found. Please complete OAuth flow first.",
                    "auth_url": "/auth"
                })),
            );
        }
        Err(e) => {
            error!("Database error retrieving token for shop {}: {}", shop, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Database error retrieving access token",
                    "details": e.to_string()
                })),
            );
        }
    };
    
    // Fetch orders from Shopify
    match fetch_orders(&token, shop).await {
        Ok(orders) => {
            info!("Successfully fetched {} orders", orders.len());
            (StatusCode::OK, Json(serde_json::json!({
                "shop": shop,
                "orders_count": orders.len(),
                "orders": orders
            })))
        }
        Err(e) => {
            error!("Failed to fetch orders: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to fetch orders",
                    "details": e.to_string()
                })),
            )
        }
    }
}

async fn fetch_orders(
    token: &str,
    shop: &str,
) -> Result<Vec<ShopifyOrder>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    
    let url = format!("https://{}/admin/api/2025-04/orders.json?limit=5&status=any", shop);
    
    info!("🔄 Fetching orders from: {}", url);
    
    let response = client
        .get(&url)
        .header("X-Shopify-Access-Token", token)
        .header("Content-Type", "application/json")
        .header("User-Agent", "Shopify OAuth Rust App")
        .send()
        .await?;
    
    let status = response.status();
    
    if !status.is_success() {
        let error_text = response.text().await?;
        error!("Shopify Orders API Error {}: {}", status, error_text);
        
        // Handle specific error cases
        match status.as_u16() {
            401 => return Err("Invalid or expired access token. Please re-authenticate.".into()),
            403 => return Err("Insufficient permissions. Check your app's scopes.".into()),
            404 => return Err("Shop not found or API endpoint unavailable.".into()),
            429 => return Err("Rate limit exceeded. Please try again later.".into()),
            _ => return Err(format!("Shopify API Error {}: {}", status, error_text).into()),
        }
    }
    
    let orders_response: OrdersResponse = response.json().await?;
    info!("✅ Successfully fetched {} orders", orders_response.orders.len());
    Ok(orders_response.orders)
}

// =============================================================================
// Application Setup and Main Function
// =============================================================================

pub async fn home_handler() -> impl IntoResponse {
    Html(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Shopify OAuth2 Demo</title>
            <style>
                body { 
                    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; 
                    max-width: 800px; 
                    margin: 0 auto; 
                    padding: 20px; 
                    line-height: 1.6;
                    color: #333;
                }
                .button { 
                    background: #5865f2; 
                    color: white; 
                    padding: 12px 24px; 
                    text-decoration: none; 
                    border-radius: 6px; 
                    display: inline-block;
                    font-weight: 500;
                    transition: background 0.2s;
                }
                .button:hover { background: #4752c4; }
                .endpoint { 
                    background: #f8f9fa; 
                    padding: 20px; 
                    margin: 15px 0; 
                    border-radius: 8px; 
                    border: 1px solid #e9ecef;
                }
                .endpoint h3 { 
                    margin-top: 0; 
                    color: #495057; 
                    font-family: 'Courier New', monospace;
                    background: #e9ecef;
                    padding: 8px 12px;
                    border-radius: 4px;
                    display: inline-block;
                }
                .try-link {
                    color: #5865f2;
                    text-decoration: none;
                    font-weight: 500;
                }
                .try-link:hover { text-decoration: underline; }
                .header { 
                    text-align: center; 
                    margin-bottom: 40px;
                }
                .emoji { font-size: 2em; margin-bottom: 10px; }
            </style>
        </head>
        <body>
            <div class="header">
                <div class="emoji">🛍️</div>
                <h1>Shopify OAuth2 Integration Demo</h1>
                <p>A proof-of-concept Shopify OAuth2 integration built with Rust and Axum.</p>
            </div>
            
            <h2>Getting Started</h2>
            <p>Click the button below to start the OAuth flow and connect your Shopify store:</p>
            <a href="/auth" class="button">🔗 Connect to Shopify</a>
            
            <h2>Available Endpoints</h2>
            <div class="endpoint">
                <h3>GET /auth</h3>
                <p>Initiates the OAuth2 flow by redirecting to Shopify's consent screen.</p>
                <p><strong>Purpose:</strong> Generates authorization URL with CSRF state and required scopes.</p>
            </div>
            
            <div class="endpoint">
                <h3>GET /callback</h3>
                <p>Handles the OAuth callback and exchanges the authorization code for an access token.</p>
                <p><strong>Purpose:</strong> Completes OAuth flow and stores access token securely.</p>
            </div>
            
            <div class="endpoint">
                <h3>GET /orders</h3>
                <p>Fetches the latest 5 orders using the stored access token.</p>
                <p><strong>Response:</strong> JSON with order details including ID, name, total price, and customer info.</p>
                <a href="/orders" class="try-link">Try it →</a>
            </div>
            
            <div class="endpoint">
                <h3>GET /abandoned-checkouts</h3>
                <p>Fetches abandoned checkouts using the stored access token.</p>
                <p><strong>Response:</strong> JSON with comprehensive checkout details including tokens, prices, timestamps, addresses, and line items.</p>
                <p><strong>Query Parameters:</strong></p>
                <ul>
                    <li><code>limit</code> - Maximum number of results (default: 50, max: 250)</li>
                    <li><code>since_id</code> - Restrict results to after specified ID</li>
                    <li><code>created_at_min/max</code> - Filter by creation date</li>
                    <li><code>updated_at_min/max</code> - Filter by update date</li>
                    <li><code>status</code> - Filter by status (default: open)</li>
                </ul>
                <a href="/abandoned-checkouts" class="try-link">Try it →</a>
                <br>
                <a href="/abandoned-checkouts?limit=10" class="try-link">Try with limit=10 →</a>
            </div>

            <div class="endpoint">
                <h3>GET /abandoned-checkouts/count</h3>
                <p>Retrieves a count of abandoned checkouts from the past 90 days.</p>
                <p><strong>Response:</strong> JSON with count of checkouts matching the filter criteria.</p>
                <p><strong>Query Parameters:</strong> Same as /abandoned-checkouts (except limit)</p>
                <a href="/abandoned-checkouts/count" class="try-link">Try it →</a>
            </div>

            <div class="endpoint">
                <h3>GET /api/products</h3>
                <p>Fetches products from your Shopify store with comprehensive filtering options.</p>
                <p><strong>Response:</strong> JSON with product details including variants, images, and inventory.</p>
                <p><strong>Query Parameters:</strong></p>
                <ul>
                    <li><code>limit</code> - Maximum number of results (default: 50, max: 250)</li>
                    <li><code>vendor</code> - Filter by vendor name</li>
                    <li><code>product_type</code> - Filter by product type</li>
                    <li><code>collection_id</code> - Filter by collection ID</li>
                    <li><code>published_status</code> - Filter by publish status (published, unpublished, any)</li>
                </ul>
                <a href="/api/products" class="try-link">Try it →</a>
                <br>
                <a href="/api/products?limit=10&published_status=published" class="try-link">Try with filters →</a>
            </div>

            <div class="endpoint">
                <h3>GET /api/customers</h3>
                <p>Fetches customer data including addresses, order history, and marketing preferences.</p>
                <p><strong>Response:</strong> JSON with customer details and address information.</p>
                <p><strong>Query Parameters:</strong></p>
                <ul>
                    <li><code>limit</code> - Maximum number of results (default: 50, max: 250)</li>
                    <li><code>since_id</code> - Restrict results to after specified ID</li>
                    <li><code>created_at_min/max</code> - Filter by creation date</li>
                    <li><code>updated_at_min/max</code> - Filter by update date</li>
                </ul>
                <a href="/api/customers" class="try-link">Try it →</a>
                <br>
                <a href="/api/customers?limit=10" class="try-link">Try with limit=10 →</a>
            </div>

            <div class="endpoint">
                <h3>GET /api/inventory</h3>
                <p>Fetches inventory levels for products across different locations.</p>
                <p><strong>Response:</strong> JSON with inventory quantities and location information.</p>
                <p><strong>Query Parameters:</strong></p>
                <ul>
                    <li><code>limit</code> - Maximum number of results (default: 50, max: 250)</li>
                    <li><code>inventory_item_ids</code> - Comma-separated list of inventory item IDs</li>
                    <li><code>location_ids</code> - Comma-separated list of location IDs</li>
                </ul>
                <a href="/api/inventory" class="try-link">Try it →</a>
            </div>

            <h2>Webhook Endpoints</h2>
            <div class="endpoint">
                <h3>POST /webhooks/*</h3>
                <p>Real-time webhook endpoints for Shopify events with HMAC verification.</p>
                <p><strong>Supported Events:</strong></p>
                <ul>
                    <li><code>/webhooks/orders/created</code> - New order notifications</li>
                    <li><code>/webhooks/orders/updated</code> - Order status changes</li>
                    <li><code>/webhooks/orders/cancelled</code> - Order cancellations</li>
                    <li><code>/webhooks/products/created</code> - New product notifications</li>
                    <li><code>/webhooks/customers/created</code> - New customer registrations</li>
                    <li><code>/webhooks/checkouts/created</code> - Abandoned checkout tracking</li>
                    <li><code>/webhooks/checkouts/updated</code> - Checkout modifications</li>
                </ul>
                <a href="/webhooks" class="try-link">View webhook configuration →</a>
            </div>

            <h2>Technical Details</h2>
            <ul>
                <li><strong>Framework:</strong> Axum (Rust async web framework)</li>
                <li><strong>OAuth2 Flow:</strong> Authorization Code Grant with CSRF protection</li>
                <li><strong>Storage:</strong> PostgreSQL with encrypted token storage</li>
                <li><strong>API Version:</strong> Shopify Admin API 2025-04</li>
                <li><strong>Security:</strong> CSRF protection, secure token storage, webhook HMAC verification</li>
                <li><strong>Rate Limiting:</strong> Redis-backed rate limiting with in-memory fallback</li>
                <li><strong>Retry Logic:</strong> Exponential backoff for failed API requests</li>
                <li><strong>Webhooks:</strong> Real-time event processing with signature verification</li>
                <li><strong>Testing:</strong> Comprehensive unit and integration tests</li>
            </ul>
        </body>
        </html>
        "#,
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize tracing for structured logging
    tracing_subscriber::fmt::init();
    
    // Load configuration from environment
    let config = AppConfig::from_env()?;
    info!("🚀 Starting Shopify OAuth2 server...");
    info!("📍 Shop: {}", config.shop);
    info!("🔗 Redirect URI: {}", config.redirect_uri);
    info!("🔑 API Key: {}...", &config.api_key[..8]);
    info!("🌍 Environment: {}", config.environment);
    
    // Create database connection pool and run migrations
    let pool = create_connection_pool(&config.database).await?;
    run_migrations(&pool).await?;
    
    // Create database-backed stores
    let token_store = DbTokenStore::new(pool.clone(), &config.database.encryption_key)?;
    let state_store = DbStateStore::new(pool.clone());
    
    // Create app state
    let app_state = AppState {
        config: config.clone(),
        token_store,
        state_store,
    };
    
    // Create rate limiting layers
    let oauth_rate_limiter = create_oauth_rate_limiter(&config.rate_limit);
    let api_rate_limiter = create_api_rate_limiter(&config.rate_limit);
    let general_rate_limiter = create_general_rate_limiter(&config.rate_limit);
    
    // Build application router with all endpoints and middleware
    let app = Router::new()
        .route("/", get(home_handler))
        // OAuth routes with specific rate limiting
        .route("/auth", get(auth_handler))
        .route("/callback", get(oauth_callback))
        .layer(oauth_rate_limiter)
        // API routes with API-specific rate limiting
        .nest("/api", Router::new()
            .route("/orders", get(orders_handler))
            .route("/abandoned-checkouts", get(abandoned_checkouts_handler))
            .route("/abandoned-checkouts/count", get(abandoned_checkouts_count_handler))
            .route("/products", get(products_handler))
            .route("/customers", get(customers_handler))
            .route("/inventory", get(inventory_handler))
            .layer(api_rate_limiter)
        )
        // Webhook routes
        .nest("/webhooks", Router::new()
            .route("/", get(list_webhooks_handler))
            .route("/orders/created", axum::routing::post(orders_created_webhook))
            .route("/orders/updated", axum::routing::post(orders_updated_webhook))
            .route("/orders/cancelled", axum::routing::post(orders_cancelled_webhook))
            .route("/products/created", axum::routing::post(products_created_webhook))
            .route("/customers/created", axum::routing::post(customers_created_webhook))
            .route("/checkouts/created", axum::routing::post(checkouts_created_webhook))
            .route("/checkouts/updated", axum::routing::post(checkouts_updated_webhook))
        )
        // Legacy routes for backward compatibility
        .route("/orders", get(orders_handler))
        .route("/abandoned-checkouts", get(abandoned_checkouts_handler))
        .route("/abandoned-checkouts/count", get(abandoned_checkouts_count_handler))
        // Global middleware layers (applied in reverse order)
        .layer(axum_middleware::from_fn(rate_limit_handler))
        .layer(axum_middleware::from_fn(security_headers_middleware))
        .layer(axum_middleware::from_fn(request_logging_middleware))
        .layer(general_rate_limiter)
        .layer(CorsLayer::permissive()) // Enable CORS for development
        .with_state(app_state);
    
    // Start background task for cleaning up expired states
    let cleanup_pool = pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300)); // Every 5 minutes
        loop {
            interval.tick().await;
            let state_store = DbStateStore::new(cleanup_pool.clone());
            if let Err(e) = state_store.cleanup_expired_states().await {
                error!("Failed to cleanup expired OAuth states: {}", e);
            }
        }
    });
    
    // Start server
    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    
    info!("🌐 Server running on http://{}", addr);
    info!("📖 Visit http://localhost:{} to get started", config.port);
    info!("🔧 Press Ctrl+C to stop the server");
    
    if config.environment == "production" {
        info!("🔒 Running in PRODUCTION mode");
        info!("⚠️  Ensure HTTPS is properly configured!");
    } else {
        info!("🛠️  Running in DEVELOPMENT mode");
    }
    
    // Serve the application
    axum::serve(listener, app).await?;
    
    Ok(())
}