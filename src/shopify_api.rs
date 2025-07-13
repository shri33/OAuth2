use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};

use crate::{AppState, http_client::ShopifyClient};

// =============================================================================
// Product Structures
// =============================================================================

#[derive(Deserialize, Serialize)]
pub struct Product {
    pub id: u64,
    pub title: String,
    pub body_html: Option<String>,
    pub vendor: String,
    pub product_type: String,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
    pub handle: String,
    pub tags: String,
    pub status: String,
    pub variants: Vec<ProductVariant>,
    pub images: Vec<ProductImage>,
    pub options: Vec<ProductOption>,
}

#[derive(Deserialize, Serialize)]
pub struct ProductVariant {
    pub id: u64,
    pub product_id: u64,
    pub title: String,
    pub price: String,
    pub sku: Option<String>,
    pub position: i32,
    pub inventory_policy: String,
    pub compare_at_price: Option<String>,
    pub fulfillment_service: String,
    pub inventory_management: Option<String>,
    pub option1: Option<String>,
    pub option2: Option<String>,
    pub option3: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub taxable: bool,
    pub barcode: Option<String>,
    pub grams: i32,
    pub image_id: Option<u64>,
    pub weight: f64,
    pub weight_unit: String,
    pub inventory_item_id: u64,
    pub inventory_quantity: i32,
    pub old_inventory_quantity: i32,
    pub requires_shipping: bool,
}

#[derive(Deserialize, Serialize)]
pub struct ProductImage {
    pub id: u64,
    pub product_id: u64,
    pub position: i32,
    pub created_at: String,
    pub updated_at: String,
    pub alt: Option<String>,
    pub width: i32,
    pub height: i32,
    pub src: String,
    pub variant_ids: Vec<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct ProductOption {
    pub id: u64,
    pub product_id: u64,
    pub name: String,
    pub position: i32,
    pub values: Vec<String>,
}

#[derive(Deserialize, Serialize)]
pub struct ProductsResponse {
    pub products: Vec<Product>,
}

#[derive(Deserialize)]
pub struct ProductParams {
    pub limit: Option<u32>,
    pub since_id: Option<u64>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub collection_id: Option<u64>,
    pub created_at_min: Option<String>,
    pub created_at_max: Option<String>,
    pub updated_at_min: Option<String>,
    pub updated_at_max: Option<String>,
    pub published_at_min: Option<String>,
    pub published_at_max: Option<String>,
    pub published_status: Option<String>,
    pub fields: Option<String>,
}

// =============================================================================
// Customer Structures
// =============================================================================

#[derive(Deserialize, Serialize)]
pub struct Customer {
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
    pub phone: Option<String>,
    pub tags: String,
    pub last_order_name: Option<String>,
    pub currency: String,
    pub addresses: Vec<CustomerAddress>,
    pub accepts_marketing_updated_at: String,
    pub marketing_opt_in_level: Option<String>,
    pub tax_exemptions: Vec<String>,
    pub email_marketing_consent: Option<EmailMarketingConsent>,
    pub sms_marketing_consent: Option<SmsMarketingConsent>,
    pub admin_graphql_api_id: String,
    pub default_address: Option<CustomerAddress>,
}

#[derive(Deserialize, Serialize)]
pub struct CustomerAddress {
    pub id: Option<u64>,
    pub customer_id: Option<u64>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub company: Option<String>,
    pub address1: Option<String>,
    pub address2: Option<String>,
    pub city: Option<String>,
    pub province: Option<String>,
    pub country: Option<String>,
    pub zip: Option<String>,
    pub phone: Option<String>,
    pub name: Option<String>,
    pub province_code: Option<String>,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    #[serde(rename = "default")]
    pub is_default: Option<bool>,
}

#[derive(Deserialize, Serialize)]
pub struct EmailMarketingConsent {
    pub state: String,
    pub opt_in_level: String,
    pub consent_updated_at: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct SmsMarketingConsent {
    pub state: String,
    pub opt_in_level: String,
    pub consent_updated_at: Option<String>,
    pub consent_collected_from: String,
}

#[derive(Deserialize, Serialize)]
pub struct CustomersResponse {
    pub customers: Vec<Customer>,
}

#[derive(Deserialize)]
pub struct CustomerParams {
    pub limit: Option<u32>,
    pub since_id: Option<u64>,
    pub created_at_min: Option<String>,
    pub created_at_max: Option<String>,
    pub updated_at_min: Option<String>,
    pub updated_at_max: Option<String>,
    pub fields: Option<String>,
}

// =============================================================================
// Inventory Structures
// =============================================================================

#[derive(Deserialize, Serialize)]
pub struct InventoryLevel {
    pub inventory_item_id: u64,
    pub location_id: u64,
    pub available: Option<i32>,
    pub updated_at: String,
}

#[derive(Deserialize, Serialize)]
pub struct InventoryLevelsResponse {
    pub inventory_levels: Vec<InventoryLevel>,
}

#[derive(Deserialize)]
pub struct InventoryParams {
    pub limit: Option<u32>,
    pub inventory_item_ids: Option<String>,
    pub location_ids: Option<String>,
    pub updated_at_min: Option<String>,
}

// =============================================================================
// API Handlers
// =============================================================================

pub async fn products_handler(
    Query(params): Query<ProductParams>,
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

    // Fetch products from Shopify
    match fetch_products(&token, shop, &params).await {
        Ok(products) => {
            info!("Successfully fetched {} products", products.len());
            (StatusCode::OK, Json(serde_json::json!({
                "shop": shop,
                "products_count": products.len(),
                "products": products
            })))
        }
        Err(e) => {
            error!("Failed to fetch products: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to fetch products",
                    "details": e.to_string()
                })),
            )
        }
    }
}

pub async fn customers_handler(
    Query(params): Query<CustomerParams>,
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

    // Fetch customers from Shopify
    match fetch_customers(&token, shop, &params).await {
        Ok(customers) => {
            info!("Successfully fetched {} customers", customers.len());
            (StatusCode::OK, Json(serde_json::json!({
                "shop": shop,
                "customers_count": customers.len(),
                "customers": customers
            })))
        }
        Err(e) => {
            error!("Failed to fetch customers: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to fetch customers",
                    "details": e.to_string()
                })),
            )
        }
    }
}

pub async fn inventory_handler(
    Query(params): Query<InventoryParams>,
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

    // Fetch inventory levels from Shopify
    match fetch_inventory_levels(&token, shop, &params).await {
        Ok(inventory_levels) => {
            info!("Successfully fetched {} inventory levels", inventory_levels.len());
            (StatusCode::OK, Json(serde_json::json!({
                "shop": shop,
                "inventory_levels_count": inventory_levels.len(),
                "inventory_levels": inventory_levels
            })))
        }
        Err(e) => {
            error!("Failed to fetch inventory levels: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to fetch inventory levels",
                    "details": e.to_string()
                })),
            )
        }
    }
}

// =============================================================================
// API Fetch Functions
// =============================================================================

async fn fetch_products(
    token: &str,
    shop: &str,
    params: &ProductParams,
) -> Result<Vec<Product>, Box<dyn std::error::Error + Send + Sync>> {
    let client = ShopifyClient::new(shop, None)?;
    
    let mut query_params = Vec::new();
    
    // Set default limit if not provided
    let limit = params.limit.unwrap_or(50);
    query_params.push(("limit", limit.to_string()));
    
    if let Some(since_id) = params.since_id {
        query_params.push(("since_id", since_id.to_string()));
    }
    
    if let Some(ref vendor) = params.vendor {
        query_params.push(("vendor", vendor.clone()));
    }
    
    if let Some(ref product_type) = params.product_type {
        query_params.push(("product_type", product_type.clone()));
    }
    
    if let Some(collection_id) = params.collection_id {
        query_params.push(("collection_id", collection_id.to_string()));
    }
    
    if let Some(ref created_at_min) = params.created_at_min {
        query_params.push(("created_at_min", created_at_min.clone()));
    }
    
    if let Some(ref created_at_max) = params.created_at_max {
        query_params.push(("created_at_max", created_at_max.clone()));
    }
    
    if let Some(ref updated_at_min) = params.updated_at_min {
        query_params.push(("updated_at_min", updated_at_min.clone()));
    }
    
    if let Some(ref updated_at_max) = params.updated_at_max {
        query_params.push(("updated_at_max", updated_at_max.clone()));
    }
    
    if let Some(ref published_at_min) = params.published_at_min {
        query_params.push(("published_at_min", published_at_min.clone()));
    }
    
    if let Some(ref published_at_max) = params.published_at_max {
        query_params.push(("published_at_max", published_at_max.clone()));
    }
    
    if let Some(ref published_status) = params.published_status {
        query_params.push(("published_status", published_status.clone()));
    }
    
    if let Some(ref fields) = params.fields {
        query_params.push(("fields", fields.clone()));
    }

    let query_params_ref: Vec<(&str, &str)> = query_params.iter()
        .map(|(k, v)| (k as &str, v as &str))
        .collect();

    let products_response: ProductsResponse = client
        .get_with_auth("products.json", token, Some(&query_params_ref))
        .await?;
    
    Ok(products_response.products)
}

async fn fetch_customers(
    token: &str,
    shop: &str,
    params: &CustomerParams,
) -> Result<Vec<Customer>, Box<dyn std::error::Error + Send + Sync>> {
    let client = ShopifyClient::new(shop, None)?;
    
    let mut query_params = Vec::new();
    
    // Set default limit if not provided
    let limit = params.limit.unwrap_or(50);
    query_params.push(("limit", limit.to_string()));
    
    if let Some(since_id) = params.since_id {
        query_params.push(("since_id", since_id.to_string()));
    }
    
    if let Some(ref created_at_min) = params.created_at_min {
        query_params.push(("created_at_min", created_at_min.clone()));
    }
    
    if let Some(ref created_at_max) = params.created_at_max {
        query_params.push(("created_at_max", created_at_max.clone()));
    }
    
    if let Some(ref updated_at_min) = params.updated_at_min {
        query_params.push(("updated_at_min", updated_at_min.clone()));
    }
    
    if let Some(ref updated_at_max) = params.updated_at_max {
        query_params.push(("updated_at_max", updated_at_max.clone()));
    }
    
    if let Some(ref fields) = params.fields {
        query_params.push(("fields", fields.clone()));
    }

    let query_params_ref: Vec<(&str, &str)> = query_params.iter()
        .map(|(k, v)| (k as &str, v as &str))
        .collect();

    let customers_response: CustomersResponse = client
        .get_with_auth("customers.json", token, Some(&query_params_ref))
        .await?;
    
    Ok(customers_response.customers)
}

async fn fetch_inventory_levels(
    token: &str,
    shop: &str,
    params: &InventoryParams,
) -> Result<Vec<InventoryLevel>, Box<dyn std::error::Error + Send + Sync>> {
    let client = ShopifyClient::new(shop, None)?;
    
    let mut query_params = Vec::new();
    
    // Set default limit if not provided
    let limit = params.limit.unwrap_or(50);
    query_params.push(("limit", limit.to_string()));
    
    if let Some(ref inventory_item_ids) = params.inventory_item_ids {
        query_params.push(("inventory_item_ids", inventory_item_ids.clone()));
    }
    
    if let Some(ref location_ids) = params.location_ids {
        query_params.push(("location_ids", location_ids.clone()));
    }
    
    if let Some(ref updated_at_min) = params.updated_at_min {
        query_params.push(("updated_at_min", updated_at_min.clone()));
    }

    let query_params_ref: Vec<(&str, &str)> = query_params.iter()
        .map(|(k, v)| (k as &str, v as &str))
        .collect();

    let inventory_response: InventoryLevelsResponse = client
        .get_with_auth("inventory_levels.json", token, Some(&query_params_ref))
        .await?;
    
    Ok(inventory_response.inventory_levels)
}

// Helper function to get token (to be implemented in main.rs)
async fn get_token(token_store: &crate::database::DbTokenStore, shop: &str) -> Option<String> {
    match token_store.get_token(shop).await {
        Ok(Some(token)) => Some(token),
        _ => None,
    }
}
