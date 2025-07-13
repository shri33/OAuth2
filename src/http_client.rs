use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, error};

// =============================================================================
// HTTP Client with Retry Logic
// =============================================================================

#[derive(Clone)]
pub struct ShopifyClient {
    client: ClientWithMiddleware,
    base_url: String,
    api_version: String,
}

impl ShopifyClient {
    pub fn new(shop_domain: &str, api_version: Option<&str>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_millis(100), Duration::from_secs(10))
            .build_with_max_retries(3);

        let client = ClientBuilder::new(Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Ok(Self {
            client,
            base_url: format!("https://{}", shop_domain),
            api_version: api_version.unwrap_or("2025-04").to_string(),
        })
    }

    pub async fn get_with_auth<T: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
        token: &str,
        query_params: Option<&[(&str, &str)]>,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let mut url = format!("{}/admin/api/{}/{}", self.base_url, self.api_version, endpoint);
        
        if let Some(params) = query_params {
            if !params.is_empty() {
                let query_string = params.iter()
                    .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
                    .collect::<Vec<_>>()
                    .join("&");
                url = format!("{}?{}", url, query_string);
            }
        }

        info!("🔄 Making Shopify API request to: {}", url);

        let response = self.client
            .get(&url)
            .header("X-Shopify-Access-Token", token)
            .header("Content-Type", "application/json")
            .header("User-Agent", "Shopify OAuth Rust App/1.0")
            .send()
            .await?;

        let status = response.status();
        
        if !status.is_success() {
            let error_text = response.text().await?;
            error!("Shopify API Error {}: {}", status, error_text);
            
            match status.as_u16() {
                401 => return Err("Invalid or expired access token. Please re-authenticate.".into()),
                403 => return Err("Insufficient permissions. Check your app's scopes.".into()),
                404 => return Err("Resource not found or API endpoint unavailable.".into()),
                429 => return Err("Rate limit exceeded. Please try again later.".into()),
                _ => return Err(format!("Shopify API Error {}: {}", status, error_text).into()),
            }
        }

        let response_json: T = response.json().await?;
        Ok(response_json)
    }

    #[allow(dead_code)]
    pub async fn post_with_auth<T: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
        token: &str,
        body: &T,
    ) -> Result<R, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/admin/api/{}/{}", self.base_url, self.api_version, endpoint);
        
        info!("🔄 Making Shopify API POST request to: {}", url);

        let response = self.client
            .post(&url)
            .header("X-Shopify-Access-Token", token)
            .header("Content-Type", "application/json")
            .header("User-Agent", "Shopify OAuth Rust App/1.0")
            .json(body)
            .send()
            .await?;

        let status = response.status();
        
        if !status.is_success() {
            let error_text = response.text().await?;
            error!("Shopify API POST Error {}: {}", status, error_text);
            return Err(format!("Shopify API POST Error {}: {}", status, error_text).into());
        }

        let response_json: R = response.json().await?;
        Ok(response_json)
    }
}

// =============================================================================
// Pagination Helper
// =============================================================================

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PaginationOptions {
    pub limit: Option<u32>,
    pub since_id: Option<u64>,
    pub page_info: Option<String>,
    pub fields: Option<Vec<String>>,
}

impl Default for PaginationOptions {
    fn default() -> Self {
        Self {
            limit: Some(50),
            since_id: None,
            page_info: None,
            fields: None,
        }
    }
}

impl PaginationOptions {
    #[allow(dead_code)]
    pub fn to_query_params(&self) -> Vec<(String, String)> {
        let mut params = Vec::new();
        
        if let Some(limit) = self.limit {
            params.push(("limit".to_string(), limit.to_string()));
        }
        
        if let Some(since_id) = self.since_id {
            params.push(("since_id".to_string(), since_id.to_string()));
        }
        
        if let Some(ref page_info) = self.page_info {
            params.push(("page_info".to_string(), page_info.clone()));
        }
        
        if let Some(ref fields) = self.fields {
            if !fields.is_empty() {
                params.push(("fields".to_string(), fields.join(",")));
            }
        }
        
        params
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PaginatedResponse<T> {
    pub data: T,
    pub page_info: Option<PageInfo>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PageInfo {
    pub has_next_page: bool,
    pub has_previous_page: bool,
    pub start_cursor: Option<String>,
    pub end_cursor: Option<String>,
}
