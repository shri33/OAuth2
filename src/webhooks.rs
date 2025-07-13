use axum::{
    extract::State,
    http::{StatusCode, HeaderMap},
    response::IntoResponse,
    Json,
    body::Bytes,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use hex;

use crate::AppState;

// =============================================================================
// Webhook Verification
// =============================================================================

type HmacSha256 = Hmac<Sha256>;

pub fn verify_webhook(
    body: &[u8],
    signature: &str,
    secret: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    // Remove 'sha256=' prefix if present
    let signature = signature.strip_prefix("sha256=").unwrap_or(signature);
    
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())?;
    mac.update(body);
    
    let expected_signature = hex::encode(mac.finalize().into_bytes());
    
    // Use constant-time comparison
    Ok(expected_signature == signature)
}

// =============================================================================
// Webhook Event Structures
// =============================================================================

#[derive(Debug, Deserialize, Serialize)]
pub struct OrderWebhook {
    pub id: u64,
    pub email: Option<String>,
    pub closed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub number: u64,
    pub note: Option<String>,
    pub token: String,
    pub gateway: Option<String>,
    pub test: bool,
    pub total_price: String,
    pub subtotal_price: String,
    pub total_weight: i32,
    pub total_tax: String,
    pub taxes_included: bool,
    pub currency: String,
    pub financial_status: String,
    pub confirmed: bool,
    pub total_discounts: String,
    pub buyer_accepts_marketing: bool,
    pub name: String,
    pub referring_site: Option<String>,
    pub landing_site: Option<String>,
    pub cancelled_at: Option<String>,
    pub cancel_reason: Option<String>,
    pub reference: Option<String>,
    pub user_id: Option<u64>,
    pub location_id: Option<u64>,
    pub source_identifier: Option<String>,
    pub source_url: Option<String>,
    pub processed_at: String,
    pub device_id: Option<u64>,
    pub phone: Option<String>,
    pub customer_locale: Option<String>,
    pub app_id: Option<u64>,
    pub browser_ip: Option<String>,
    pub landing_site_ref: Option<String>,
    pub order_number: u64,
    pub processing_method: String,
    pub checkout_id: Option<u64>,
    pub source_name: String,
    pub fulfillment_status: Option<String>,
    pub tax_lines: Vec<serde_json::Value>,
    pub tags: String,
    pub contact_email: Option<String>,
    pub order_status_url: String,
    pub presentment_currency: String,
    pub total_line_items_price: String,
    pub total_discounts_set: serde_json::Value,
    pub total_line_items_price_set: serde_json::Value,
    pub total_price_set: serde_json::Value,
    pub total_shipping_price_set: serde_json::Value,
    pub subtotal_price_set: serde_json::Value,
    pub total_tax_set: serde_json::Value,
    pub line_items: Vec<serde_json::Value>,
    pub fulfillments: Vec<serde_json::Value>,
    pub refunds: Vec<serde_json::Value>,
    pub customer: Option<serde_json::Value>,
    pub billing_address: Option<serde_json::Value>,
    pub shipping_address: Option<serde_json::Value>,
    pub shipping_lines: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProductWebhook {
    pub id: u64,
    pub title: String,
    pub body_html: Option<String>,
    pub vendor: String,
    pub product_type: String,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
    pub template_suffix: Option<String>,
    pub published_scope: String,
    pub tags: String,
    pub status: String,
    pub admin_graphql_api_id: String,
    pub variants: Vec<serde_json::Value>,
    pub options: Vec<serde_json::Value>,
    pub images: Vec<serde_json::Value>,
    pub image: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CustomerWebhook {
    pub id: u64,
    pub email: Option<String>,
    pub accepts_marketing: bool,
    pub created_at: String,
    pub updated_at: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub orders_count: i32,
    pub state: String,
    pub total_spent: String,
    pub last_order_id: Option<u64>,
    pub note: Option<String>,
    pub verified_email: bool,
    pub multipass_identifier: Option<String>,
    pub tax_exempt: bool,
    pub tags: String,
    pub last_order_name: Option<String>,
    pub currency: String,
    pub phone: Option<String>,
    pub addresses: Vec<serde_json::Value>,
    pub admin_graphql_api_id: String,
    pub default_address: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CheckoutWebhook {
    pub id: u64,
    pub token: String,
    pub cart_token: Option<String>,
    pub email: Option<String>,
    pub gateway: Option<String>,
    pub buyer_accepts_marketing: Option<bool>,
    pub created_at: String,
    pub updated_at: String,
    pub landing_site: Option<String>,
    pub note: Option<String>,
    pub note_attributes: Vec<serde_json::Value>,
    pub referring_site: Option<String>,
    pub shipping_lines: Vec<serde_json::Value>,
    pub taxes_included: bool,
    pub total_weight: i32,
    pub currency: String,
    pub completed_at: Option<String>,
    pub closed_at: Option<String>,
    pub user_id: Option<u64>,
    pub location_id: Option<u64>,
    pub source_identifier: Option<String>,
    pub source_url: Option<String>,
    pub device_id: Option<u64>,
    pub phone: Option<String>,
    pub customer_locale: Option<String>,
    pub line_items: Vec<serde_json::Value>,
    pub name: String,
    pub source: Option<String>,
    pub abandoned_checkout_url: String,
    pub discount_codes: Vec<serde_json::Value>,
    pub tax_lines: Vec<serde_json::Value>,
    pub source_name: String,
    pub presentment_currency: String,
    pub buyer_accepts_sms_marketing: Option<bool>,
    pub sms_marketing_phone: Option<String>,
    pub total_discounts: String,
    pub total_line_items_price: String,
    pub total_price: String,
    pub total_tax: String,
    pub subtotal_price: String,
    pub billing_address: Option<serde_json::Value>,
    pub shipping_address: Option<serde_json::Value>,
    pub customer: Option<serde_json::Value>,
}

// =============================================================================
// Webhook Response Structures
// =============================================================================

#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub status: String,
    pub message: String,
    pub timestamp: String,
    pub webhook_id: Option<String>,
}

impl WebhookResponse {
    pub fn success(message: &str) -> Self {
        Self {
            status: "success".to_string(),
            message: message.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            webhook_id: None,
        }
    }

    pub fn error(message: &str) -> Self {
        Self {
            status: "error".to_string(),
            message: message.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            webhook_id: None,
        }
    }
}

// =============================================================================
// Webhook Handlers
// =============================================================================

pub async fn orders_created_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    debug!("Received order created webhook");
    
    // Verify webhook authenticity
    if let Err(e) = verify_webhook_request(&headers, &body, &state.config.api_secret).await {
        warn!("Webhook verification failed: {}", e);
        return (
            StatusCode::UNAUTHORIZED,
            Json(WebhookResponse::error("Webhook verification failed")),
        );
    }

    // Parse the order data
    match serde_json::from_slice::<OrderWebhook>(&body) {
        Ok(order) => {
            info!("✅ Order created: {} - ${} - {}", order.name, order.total_price, order.email.unwrap_or_default());
            
            // Here you would typically:
            // 1. Store the order in your database
            // 2. Send notifications
            // 3. Trigger business logic
            // 4. Update inventory tracking
            // 5. Send confirmation emails
            
            // For now, just log the event
            info!("Order {} processed successfully", order.id);
            
            (
                StatusCode::OK,
                Json(WebhookResponse::success(&format!("Order {} processed", order.id))),
            )
        }
        Err(e) => {
            error!("Failed to parse order webhook: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(WebhookResponse::error("Failed to parse order data")),
            )
        }
    }
}

pub async fn orders_updated_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    debug!("Received order updated webhook");
    
    if let Err(e) = verify_webhook_request(&headers, &body, &state.config.api_secret).await {
        warn!("Webhook verification failed: {}", e);
        return (
            StatusCode::UNAUTHORIZED,
            Json(WebhookResponse::error("Webhook verification failed")),
        );
    }

    match serde_json::from_slice::<OrderWebhook>(&body) {
        Ok(order) => {
            info!("📝 Order updated: {} - Status: {}", order.name, order.financial_status);
            
            // Handle order update logic here
            
            (
                StatusCode::OK,
                Json(WebhookResponse::success(&format!("Order {} update processed", order.id))),
            )
        }
        Err(e) => {
            error!("Failed to parse order update webhook: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(WebhookResponse::error("Failed to parse order update data")),
            )
        }
    }
}

pub async fn orders_cancelled_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    debug!("Received order cancelled webhook");
    
    if let Err(e) = verify_webhook_request(&headers, &body, &state.config.api_secret).await {
        warn!("Webhook verification failed: {}", e);
        return (
            StatusCode::UNAUTHORIZED,
            Json(WebhookResponse::error("Webhook verification failed")),
        );
    }

    match serde_json::from_slice::<OrderWebhook>(&body) {
        Ok(order) => {
            info!("❌ Order cancelled: {} - Reason: {}", order.name, order.cancel_reason.unwrap_or_default());
            
            // Handle order cancellation logic here
            
            (
                StatusCode::OK,
                Json(WebhookResponse::success(&format!("Order {} cancellation processed", order.id))),
            )
        }
        Err(e) => {
            error!("Failed to parse order cancellation webhook: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(WebhookResponse::error("Failed to parse order cancellation data")),
            )
        }
    }
}

pub async fn products_created_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    debug!("Received product created webhook");
    
    if let Err(e) = verify_webhook_request(&headers, &body, &state.config.api_secret).await {
        warn!("Webhook verification failed: {}", e);
        return (
            StatusCode::UNAUTHORIZED,
            Json(WebhookResponse::error("Webhook verification failed")),
        );
    }

    match serde_json::from_slice::<ProductWebhook>(&body) {
        Ok(product) => {
            info!("🆕 Product created: {} - {}", product.title, product.vendor);
            
            // Handle product creation logic here
            
            (
                StatusCode::OK,
                Json(WebhookResponse::success(&format!("Product {} processed", product.id))),
            )
        }
        Err(e) => {
            error!("Failed to parse product webhook: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(WebhookResponse::error("Failed to parse product data")),
            )
        }
    }
}

pub async fn customers_created_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    debug!("Received customer created webhook");
    
    if let Err(e) = verify_webhook_request(&headers, &body, &state.config.api_secret).await {
        warn!("Webhook verification failed: {}", e);
        return (
            StatusCode::UNAUTHORIZED,
            Json(WebhookResponse::error("Webhook verification failed")),
        );
    }

    match serde_json::from_slice::<CustomerWebhook>(&body) {
        Ok(customer) => {
            info!("👤 Customer created: {} {} - {}", 
                customer.first_name.unwrap_or_default(),
                customer.last_name.unwrap_or_default(),
                customer.email.unwrap_or_default()
            );
            
            // Handle customer creation logic here
            
            (
                StatusCode::OK,
                Json(WebhookResponse::success(&format!("Customer {} processed", customer.id))),
            )
        }
        Err(e) => {
            error!("Failed to parse customer webhook: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(WebhookResponse::error("Failed to parse customer data")),
            )
        }
    }
}

pub async fn checkouts_created_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    debug!("Received checkout created webhook");
    
    if let Err(e) = verify_webhook_request(&headers, &body, &state.config.api_secret).await {
        warn!("Webhook verification failed: {}", e);
        return (
            StatusCode::UNAUTHORIZED,
            Json(WebhookResponse::error("Webhook verification failed")),
        );
    }

    match serde_json::from_slice::<CheckoutWebhook>(&body) {
        Ok(checkout) => {
            info!("🛒 Checkout created: {} - ${}", checkout.token, checkout.total_price);
            
            // Handle checkout creation logic here
            
            (
                StatusCode::OK,
                Json(WebhookResponse::success(&format!("Checkout {} processed", checkout.id))),
            )
        }
        Err(e) => {
            error!("Failed to parse checkout webhook: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(WebhookResponse::error("Failed to parse checkout data")),
            )
        }
    }
}

pub async fn checkouts_updated_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    debug!("Received checkout updated webhook");
    
    if let Err(e) = verify_webhook_request(&headers, &body, &state.config.api_secret).await {
        warn!("Webhook verification failed: {}", e);
        return (
            StatusCode::UNAUTHORIZED,
            Json(WebhookResponse::error("Webhook verification failed")),
        );
    }

    match serde_json::from_slice::<CheckoutWebhook>(&body) {
        Ok(checkout) => {
            info!("📝 Checkout updated: {} - ${}", checkout.token, checkout.total_price);
            
            // Handle checkout update logic here
            
            (
                StatusCode::OK,
                Json(WebhookResponse::success(&format!("Checkout {} update processed", checkout.id))),
            )
        }
        Err(e) => {
            error!("Failed to parse checkout update webhook: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(WebhookResponse::error("Failed to parse checkout update data")),
            )
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

async fn verify_webhook_request(
    headers: &HeaderMap,
    body: &[u8],
    secret: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let signature = headers
        .get("X-Shopify-Hmac-Sha256")
        .and_then(|v| v.to_str().ok())
        .ok_or("Missing X-Shopify-Hmac-Sha256 header")?;

    if !verify_webhook(body, signature, secret)? {
        return Err("Invalid webhook signature".into());
    }

    // Additional verification: check shop domain if available
    if let Some(shop_domain) = headers.get("X-Shopify-Shop-Domain").and_then(|v| v.to_str().ok()) {
        debug!("Webhook from shop: {}", shop_domain);
        // You could add additional validation here to ensure the webhook is from the expected shop
    }

    Ok(())
}

// Webhook management endpoint to list configured webhooks
pub async fn list_webhooks_handler(
    State(_state): State<AppState>,
) -> impl IntoResponse {
    // This would typically fetch webhooks from Shopify API
    // For now, return the endpoints this app supports
    
    let supported_webhooks = serde_json::json!({
        "supported_webhooks": [
            {
                "topic": "orders/create",
                "endpoint": "/webhooks/orders/created",
                "description": "Triggered when a new order is created"
            },
            {
                "topic": "orders/updated",
                "endpoint": "/webhooks/orders/updated", 
                "description": "Triggered when an order is updated"
            },
            {
                "topic": "orders/cancelled",
                "endpoint": "/webhooks/orders/cancelled",
                "description": "Triggered when an order is cancelled"
            },
            {
                "topic": "products/create",
                "endpoint": "/webhooks/products/created",
                "description": "Triggered when a new product is created"
            },
            {
                "topic": "customers/create",
                "endpoint": "/webhooks/customers/created",
                "description": "Triggered when a new customer is created"
            },
            {
                "topic": "checkouts/create",
                "endpoint": "/webhooks/checkouts/created",
                "description": "Triggered when a new checkout is created"
            },
            {
                "topic": "checkouts/update",
                "endpoint": "/webhooks/checkouts/updated",
                "description": "Triggered when a checkout is updated"
            }
        ],
        "webhook_verification": "HMAC SHA256 with API secret",
        "format": "JSON"
    });

    (StatusCode::OK, Json(supported_webhooks))
}
