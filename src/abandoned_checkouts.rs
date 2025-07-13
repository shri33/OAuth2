use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};

use crate::{AppState, get_token};

// Shopify Address structure
#[derive(Deserialize, Serialize)]
pub struct Address {
    pub address1: Option<String>,
    pub address2: Option<String>,
    pub city: Option<String>,
    pub company: Option<String>,
    pub country: Option<String>,
    pub country_code: Option<String>,
    #[serde(rename = "default")]
    pub is_default: Option<bool>,
    pub first_name: Option<String>,
    pub id: Option<u64>,
    pub last_name: Option<String>,
    pub name: Option<String>,
    pub phone: Option<String>,
    pub province: Option<String>,
    pub province_code: Option<String>,
    pub zip: Option<String>,
}

// Shopify Customer structure
#[derive(Deserialize, Serialize)]
pub struct Customer {
    pub id: Option<u64>,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub accepts_marketing: Option<bool>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub note: Option<String>,
    pub phone: Option<String>,
}

// Shopify Line Item structure
#[derive(Deserialize, Serialize)]
pub struct LineItem {
    pub id: Option<u64>,
    pub product_id: Option<u64>,
    pub variant_id: Option<u64>,
    pub title: Option<String>,
    pub variant_title: Option<String>,
    pub sku: Option<String>,
    pub vendor: Option<String>,
    pub quantity: Option<i32>,
    pub price: Option<String>,
    pub total_discount: Option<String>,
}

// Shopify Abandoned Checkout structure (comprehensive)
#[derive(Deserialize, Serialize)]
pub struct AbandonedCheckout {
    pub id: u64,
    pub token: String,
    pub cart_token: Option<String>,
    pub email: Option<String>,
    pub gateway: Option<String>,
    pub buyer_accepts_marketing: Option<bool>,
    pub buyer_accepts_sms_marketing: Option<bool>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub closed_at: Option<String>,
    pub currency: Option<String>,
    pub presentment_currency: Option<String>,
    pub total_price: Option<String>,
    pub total_tax: Option<String>,
    pub total_line_items_price: Option<String>,
    pub subtotal_price: Option<String>,
    pub total_discounts: Option<String>,
    pub abandoned_checkout_url: Option<String>,
    pub billing_address: Option<Address>,
    pub shipping_address: Option<Address>,
    pub customer: Option<Customer>,
    pub customer_locale: Option<String>,
    pub line_items: Option<Vec<LineItem>>,
    pub landing_site: Option<String>,
    pub referring_site: Option<String>,
    pub note: Option<String>,
    pub source_name: Option<String>,
    pub phone: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct AbandonedCheckoutsResponse {
    pub checkouts: Vec<AbandonedCheckout>,
}

// Query parameters for abandoned checkouts
#[derive(Deserialize)]
pub struct AbandonedCheckoutParams {
    pub limit: Option<i32>,
    pub since_id: Option<u64>,
    pub created_at_min: Option<String>,
    pub created_at_max: Option<String>,
    pub updated_at_min: Option<String>,
    pub updated_at_max: Option<String>,
    pub status: Option<String>,
}

pub async fn abandoned_checkouts_handler(
    Query(params): Query<AbandonedCheckoutParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let shop = &state.config.shop;
    
    // Get stored access token
    let token = match get_token(&state.token_store, shop).await {
        Some(token) => token,
        None => {
            warn!("No access token found for shop: {}", shop);
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "No access token found. Please complete OAuth flow first.",
                    "auth_url": "/auth"
                })),
            );
        }
    };
    
    // Fetch abandoned checkouts from Shopify
    match fetch_abandoned_checkouts(&token, shop, &params).await {
        Ok(checkouts) => {
            info!("Successfully fetched {} abandoned checkouts", checkouts.len());
            (StatusCode::OK, Json(serde_json::json!({
                "shop": shop,
                "checkouts_count": checkouts.len(),
                "abandoned_checkouts": checkouts
            })))
        }
        Err(e) => {
            error!("Failed to fetch abandoned checkouts: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to fetch abandoned checkouts",
                    "details": e.to_string()
                })),
            )
        }
    }
}

async fn fetch_abandoned_checkouts(
    token: &str,
    shop: &str,
    params: &AbandonedCheckoutParams,
) -> Result<Vec<AbandonedCheckout>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    // Build query parameters
    let mut query_params = Vec::new();
    
    // Set default limit if not provided
    let limit = params.limit.unwrap_or(50);
    query_params.push(format!("limit={}", limit));
    
    if let Some(since_id) = params.since_id {
        query_params.push(format!("since_id={}", since_id));
    }
    
    if let Some(ref created_at_min) = params.created_at_min {
        query_params.push(format!("created_at_min={}", urlencoding::encode(created_at_min)));
    }
    
    if let Some(ref created_at_max) = params.created_at_max {
        query_params.push(format!("created_at_max={}", urlencoding::encode(created_at_max)));
    }
    
    if let Some(ref updated_at_min) = params.updated_at_min {
        query_params.push(format!("updated_at_min={}", urlencoding::encode(updated_at_min)));
    }
    
    if let Some(ref updated_at_max) = params.updated_at_max {
        query_params.push(format!("updated_at_max={}", urlencoding::encode(updated_at_max)));
    }
    
    if let Some(ref status) = params.status {
        query_params.push(format!("status={}", status));
    }
    
    let query_string = if query_params.is_empty() {
        String::new()
    } else {
        format!("?{}", query_params.join("&"))
    };
    
    let url = format!("https://{}/admin/api/2025-04/checkouts.json{}", shop, query_string);
    
    let response = client
        .get(&url)
        .header("X-Shopify-Access-Token", token)
        .header("Content-Type", "application/json")
        .send()
        .await?;
    
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await?;
        return Err(format!("Shopify API Error {}: {}", status, error_text).into());
    }
    
    let checkouts_response: AbandonedCheckoutsResponse = response.json().await?;
    Ok(checkouts_response.checkouts)
}

// New endpoint to get abandoned checkouts count
pub async fn abandoned_checkouts_count_handler(
    Query(params): Query<AbandonedCheckoutParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let shop = &state.config.shop;
    
    // Get stored access token
    let token = match get_token(&state.token_store, shop).await {
        Some(token) => token,
        None => {
            warn!("No access token found for shop: {}", shop);
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "No access token found. Please complete OAuth flow first.",
                    "auth_url": "/auth"
                })),
            );
        }
    };
    
    // Fetch count of abandoned checkouts from Shopify
    match fetch_abandoned_checkouts_count(&token, shop, &params).await {
        Ok(count) => {
            info!("Successfully fetched abandoned checkouts count: {}", count);
            (StatusCode::OK, Json(serde_json::json!({
                "shop": shop,
                "count": count
            })))
        }
        Err(e) => {
            error!("Failed to fetch abandoned checkouts count: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to fetch abandoned checkouts count",
                    "details": e.to_string()
                })),
            )
        }
    }
}

async fn fetch_abandoned_checkouts_count(
    token: &str,
    shop: &str,
    params: &AbandonedCheckoutParams,
) -> Result<u64, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    // Build query parameters (same as regular fetch but for count endpoint)
    let mut query_params = Vec::new();
    
    if let Some(since_id) = params.since_id {
        query_params.push(format!("since_id={}", since_id));
    }
    
    if let Some(ref created_at_min) = params.created_at_min {
        query_params.push(format!("created_at_min={}", urlencoding::encode(created_at_min)));
    }
    
    if let Some(ref created_at_max) = params.created_at_max {
        query_params.push(format!("created_at_max={}", urlencoding::encode(created_at_max)));
    }
    
    if let Some(ref updated_at_min) = params.updated_at_min {
        query_params.push(format!("updated_at_min={}", urlencoding::encode(updated_at_min)));
    }
    
    if let Some(ref updated_at_max) = params.updated_at_max {
        query_params.push(format!("updated_at_max={}", urlencoding::encode(updated_at_max)));
    }
    
    if let Some(ref status) = params.status {
        query_params.push(format!("status={}", status));
    }
    
    let query_string = if query_params.is_empty() {
        String::new()
    } else {
        format!("?{}", query_params.join("&"))
    };
    
    let url = format!("https://{}/admin/api/2025-04/checkouts/count.json{}", shop, query_string);
    
    let response = client
        .get(&url)
        .header("X-Shopify-Access-Token", token)
        .header("Content-Type", "application/json")
        .send()
        .await?;
    
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await?;
        return Err(format!("Shopify API Error {}: {}", status, error_text).into());
    }
    
    let count_response: serde_json::Value = response.json().await?;
    let count = count_response["count"].as_u64().unwrap_or(0);
    Ok(count)
}
