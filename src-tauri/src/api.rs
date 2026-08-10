use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct MealieRecipeResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// A clean HTTP bridge structure to securely interface with Mealie REST endpoints
pub struct MealieClient {
    client: Client,
    base_url: String,
    token: String,
}

impl MealieClient {
    pub fn new(base_url: String, token: String) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            token,
        }
    }

    /// Pull the entire recipe list catalog metadata from the Mealie instance
    pub async fn fetch_all_recipes(&self) -> Result<Vec<MealieRecipeResponse>, String> {
        let url = format!("{}/api/recipes", self.base_url);

        let response = self.client.get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Network request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Server returned error code: {}", response.status()));
        }

        response.json::<Vec<MealieRecipeResponse>>()
            .await
            .map_err(|e| format!("Failed to parse recipe response data: {}", e))
    }
}
