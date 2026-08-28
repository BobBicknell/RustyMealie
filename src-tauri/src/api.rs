use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct MealieTag {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct MealieRecipeSummary {
    pub id: Option<String>,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<MealieTag>>,
    /// Mealie's summary/detail payloads call this `recipeCategory`.
    #[serde(default, rename = "recipeCategory", alias = "categories")]
    pub categories: Option<Vec<MealieTag>>,
}

impl MealieRecipeSummary {
    pub fn tag_names(&self) -> Vec<String> {
        self.tags
            .as_ref()
            .map(|list| list.iter().map(|t| t.name.clone()).collect())
            .unwrap_or_default()
    }

    pub fn category_names(&self) -> Vec<String> {
        self.categories
            .as_ref()
            .map(|list| list.iter().map(|c| c.name.clone()).collect())
            .unwrap_or_default()
    }
}

#[derive(Debug, Deserialize)]
struct MealiePage<T: std::fmt::Debug> {
    #[serde(default)]
    total_pages: i64,
    items: Vec<T>,
}

/// A single shopping-list item as served by Mealie. The summary page and
/// the list detail both embed these (the detail embeds them under `listItems`).
#[derive(Debug, Deserialize, Serialize)]
pub struct MealieShoppingListItem {
    pub id: Option<String>,
    pub display: Option<String>,
    pub note: Option<String>,
    #[serde(default)]
    pub checked: bool,
    #[serde(default)]
    pub position: i64,
}

/// A list summary row from `GET /api/households/shopping/lists`.
#[derive(Debug, Deserialize)]
pub struct MealieShoppingListSummary {
    pub id: Option<String>,
    pub name: Option<String>,
}

/// Full shopping-list detail (`GET /api/households/shopping/lists/{id}`),
/// including the items belonging to the list.
#[derive(Debug, Deserialize, Serialize)]
pub struct MealieShoppingList {
    pub id: Option<String>,
    pub name: Option<String>,
    #[serde(default, rename = "listItems")]
    pub list_items: Vec<MealieShoppingListItem>,
}

const PAGE_SIZE: usize = 100;

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

    pub async fn fetch_all_recipe_summaries(
        &self,
    ) -> Result<Vec<MealieRecipeSummary>, String> {
        let mut all = Vec::new();
        let mut page: i64 = 1;

        loop {
            let url = format!("{}/api/recipes", self.base_url);
            let response = self
                .client
                .get(&url)
                .query(&[("page", page.to_string()), ("perPage", PAGE_SIZE.to_string())])
                .bearer_auth(&self.token)
                .send()
                .await
                .map_err(|e| format!("Network request failed: {}", e))?;

            if !response.status().is_success() {
                return Err(format!("Server returned error code: {}", response.status()));
            }

            let body = response
                .json::<MealiePage<MealieRecipeSummary>>()
                .await
                .map_err(|e| format!("Failed to parse recipe response data: {}", e))?;

            let count = body.items.len();
            all.extend(body.items);

            if count < PAGE_SIZE || (body.total_pages > 0 && page >= body.total_pages) {
                break;
            }
            page += 1;
        }

        Ok(all)
    }

    pub async fn fetch_recipe_detail(&self, slug: &str) -> Result<Value, String> {
        let url = format!("{}/api/recipes/{}", self.base_url, slug);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Network request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!(
                "Server returned error code {} for recipe '{}'",
                response.status(),
                slug
            ));
        }

        response
            .json::<Value>()
            .await
            .map_err(|e| format!("Failed to parse recipe detail data: {}", e))
    }

    pub async fn download_image(&self, recipe_id: &str, file_name: &str) -> Result<Vec<u8>, String> {
        let url = format!(
            "{}/api/media/recipes/{}/images/{}",
            self.base_url, recipe_id, file_name
        );

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Network request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!(
                "Server returned error code {} for image '{}'",
                response.status(),
                file_name
            ));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read image data: {}", e))?;
        Ok(bytes.to_vec())
    }

    /// Download a recipe's photo. Mealie serves standardized resolution
    /// names (`min-original.webp`, `tiny-original.webp`) that map to the
    /// underlying stored file regardless of the internal filename reported
    /// in the recipe summary, so we request those instead of feeding the
    /// raw value into the URL. Fall back to the original if a resized
    /// variant isn't generated for a given image.
    pub async fn download_recipe_image(
        &self,
        recipe_id: &str,
        preferred: &str,
    ) -> Result<Vec<u8>, String> {
        for file_name in [preferred, "original.webp"] {
            if let Ok(bytes) = self.download_image(recipe_id, file_name).await {
                return Ok(bytes);
            }
        }
        Err(format!("No image available for recipe '{recipe_id}'"))
    }

    /// Fetch every shopping list the household has access to, paging
    /// through the collection like the recipe list does.
    pub async fn fetch_shopping_lists(
        &self,
    ) -> Result<Vec<MealieShoppingListSummary>, String> {
        let mut all = Vec::new();
        let mut page: i64 = 1;

        loop {
            let url = format!("{}/api/households/shopping/lists", self.base_url);
            let response = self
                .client
                .get(&url)
                .query(&[
                    ("orderBy", "name".to_string()),
                    ("orderDirection", "asc".to_string()),
                    ("page", page.to_string()),
                    ("perPage", PAGE_SIZE.to_string()),
                ])
                .bearer_auth(&self.token)
                .send()
                .await
                .map_err(|e| format!("Network request failed: {}", e))?;

            let body = response
                .error_for_status()
                .map_err(|e| format!("Failed to fetch shopping lists: {}", e))?
                .json::<MealiePage<MealieShoppingListSummary>>()
                .await
                .map_err(|e| format!("Failed to parse shopping list response: {}", e))?;

            let count = body.items.len();
            all.extend(body.items);

            if count < PAGE_SIZE || (body.total_pages > 0 && page >= body.total_pages) {
                break;
            }
            page += 1;
        }

        Ok(all)
    }

    /// Fetch one shopping list with its items.
    pub async fn fetch_shopping_list(
        &self,
        list_id: &str,
    ) -> Result<MealieShoppingList, String> {
        let url = format!(
            "{}/api/households/shopping/lists/{}",
            self.base_url, list_id
        );

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Network request failed: {}", e))?;

        response
            .error_for_status()
            .map_err(|e| format!("Failed to fetch shopping list '{list_id}': {}", e))?
            .json::<MealieShoppingList>()
            .await
            .map_err(|e| format!("Failed to parse shopping list detail: {}", e))
    }

    /// Add every ingredient of a recipe to a shopping list, letting the
    /// server handle quantity/unit parsing and deduplication. Returns the
    /// resulting list so the caller can re-cache it.
    pub async fn add_recipe_to_shopping_list(
        &self,
        list_id: &str,
        recipe_id: &str,
    ) -> Result<MealieShoppingList, String> {
        let url = format!(
            "{}/api/households/shopping/lists/{}/recipe/{}",
            self.base_url, list_id, recipe_id
        );

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .json(&json!({}))
            .send()
            .await
            .map_err(|e| format!("Network request failed: {}", e))?;

        response
            .error_for_status()
            .map_err(|e| format!("Failed to add recipe ingredients: {}", e))?
            .json::<MealieShoppingList>()
            .await
            .map_err(|e| format!("Failed to parse added shopping items: {}", e))
    }

    /// Create a new item on a shopping list. Only the note is supplied;
    /// Mealie computes the `display` text from it. Returns the created item.
    pub async fn create_shopping_list_item(
        &self,
        list_id: &str,
        note: &str,
    ) -> Result<MealieShoppingListItem, String> {
        let url = format!("{}/api/households/shopping/items", self.base_url);
        let body = json!({ "note": note, "shoppingListId": list_id });

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Network request failed: {}", e))?;

        let created = response
            .error_for_status()
            .map_err(|e| format!("Failed to add shopping item: {}", e))?
            .json::<Value>()
            .await
            .map_err(|e| format!("Failed to parse create response: {}", e))?;

        let item = created
            .get("createdItems")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .ok_or_else(|| "Server did not return the created item".to_string())?;

        serde_json::from_value(item.clone())
            .map_err(|e| format!("Failed to parse created shopping item: {}", e))
    }

    /// Flip the `checked` flag on a shopping-list item. Mealie's item
    /// update is a full PUT, so we fetch the current item and echo its
    /// editable fields back with only `checked` changed. Returns the item
    /// as it was saved.
    pub async fn set_shopping_item_checked(
        &self,
        item_id: &str,
        checked: bool,
    ) -> Result<MealieShoppingListItem, String> {
        let url = format!("{}/api/households/shopping/items/{}", self.base_url, item_id);

        let current = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Network request failed: {}", e))?
            .error_for_status()
            .map_err(|e| format!("Failed to fetch shopping item '{item_id}': {}", e))?
            .json::<Value>()
            .await
            .map_err(|e| format!("Failed to parse shopping item: {}", e))?;

        let body = shopping_item_update_body(&current, checked);

        let response = self
            .client
            .put(&url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Network request failed: {}", e))?;

        let updated = response
            .error_for_status()
            .map_err(|e| format!("Failed to update shopping item: {}", e))?
            .json::<Value>()
            .await
            .map_err(|e| format!("Failed to parse update response: {}", e))?;

        let item = updated
            .get("updatedItems")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .ok_or_else(|| "Server did not return the updated item".to_string())?;

        serde_json::from_value(item.clone())
            .map_err(|e| format!("Failed to parse updated shopping item: {}", e))
    }
}

/// Editable fields accepted by the item PUT endpoint. We echo these from
/// the fetched item so a single `checked` flip doesn't clobber the rest.
const SHOPPING_ITEM_EDITABLE: &[&str] = &[
    "shoppingListId",
    "checked",
    "display",
    "note",
    "position",
    "quantity",
    "extras",
    "food",
    "unit",
    "foodId",
    "unitId",
    "labelId",
    "referencedRecipe",
    "recipeReferences",
];

/// Build the PUT body for a shopping item: the fetched item reduced to its
/// editable fields, with `checked` forced to the requested value.
pub fn shopping_item_update_body(item: &Value, checked: bool) -> Value {
    let mut body = serde_json::Map::new();
    if let Some(obj) = item.as_object() {
        for key in SHOPPING_ITEM_EDITABLE {
            if let Some(value) = obj.get(*key) {
                body.insert((*key).to_string(), value.clone());
            }
        }
    }
    body.insert("checked".to_string(), Value::Bool(checked));
    Value::Object(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_v3_pagination_envelope_with_tag_objects() {
        let payload = r#"{
            "page": 1,
            "per_page": 100,
            "total": 2,
            "total_pages": 1,
            "next": null,
            "previous": null,
            "items": [
                {
                    "id": "b6e7d3a5-1111-2222-3333-444455556666",
                    "userId": "u",
                    "householdId": "h",
                    "groupId": "g",
                    "name": "Pasta Carbonara",
                    "slug": "pasta-carbonara",
                    "image": "original.webp",
                    "description": "Classic Roman pasta",
                    "tags": [{ "id": null, "groupId": null, "name": "italian", "slug": "italian" }],
                    "recipeCategory": [{ "name": "main course", "slug": "main-course" }]
                },
                {
                    "id": "9999",
                    "name": "No Image Recipe",
                    "slug": "no-image-recipe"
                }
            ]
        }"#;

        let page: MealiePage<MealieRecipeSummary> = serde_json::from_str(payload).unwrap();
        assert_eq!(page.total_pages, 1);
        assert_eq!(page.items.len(), 2);

        let first = &page.items[0];
        assert_eq!(first.id.as_deref(), Some("b6e7d3a5-1111-2222-3333-444455556666"));
        assert_eq!(first.slug.as_deref(), Some("pasta-carbonara"));
        assert_eq!(first.image.as_deref(), Some("original.webp"));
        assert_eq!(first.tag_names(), vec!["italian".to_string()]);
        assert_eq!(first.category_names(), vec!["main course".to_string()]);

        let second = &page.items[1];
        assert_eq!(second.image, None);
        assert_eq!(second.tag_names(), Vec::<String>::new());
        assert_eq!(second.category_names(), Vec::<String>::new());
    }

    #[test]
    fn parses_shopping_list_summary_collection() {
        let payload = r#"{
            "page": 1,
            "per_page": 10,
            "total": 2,
            "total_pages": 1,
            "items": [
                { "id": "list-1", "name": "Shopping list", "householdId": "h" },
                { "id": "list-2", "name": "Weekly staples" }
            ]
        }"#;

        let page: MealiePage<MealieShoppingListSummary> = serde_json::from_str(payload).unwrap();
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[1].id.as_deref(), Some("list-2"));
        assert_eq!(page.items[1].name.as_deref(), Some("Weekly staples"));
    }

    #[test]
    fn parses_shopping_list_detail_with_items() {
        let payload = r#"{
            "id": "list-1",
            "name": "Shopping list",
            "listItems": [
                {
                    "id": "item-1",
                    "display": "2 cups milk",
                    "note": "whole milk",
                    "checked": false,
                    "position": 1,
                    "quantity": 2.0
                },
                {
                    "id": "item-2",
                    "display": "4 eggs",
                    "checked": true,
                    "position": 0
                }
            ]
        }"#;

        let list: MealieShoppingList = serde_json::from_str(payload).unwrap();
        assert_eq!(list.id.as_deref(), Some("list-1"));
        assert_eq!(list.list_items.len(), 2);
        assert!(!list.list_items[0].checked);
        assert_eq!(list.list_items[0].position, 1);
        assert_eq!(list.list_items[0].note.as_deref(), Some("whole milk"));
        assert_eq!(list.list_items[0].display.as_deref(), Some("2 cups milk"));
        assert!(list.list_items[1].checked);
        assert_eq!(list.list_items[1].position, 0);
    }

    #[test]
    fn shopping_item_update_body_echoes_editable_fields_and_forces_checked() {
        let item = json!({
            "id": "item-1",
            "shoppingListId": "list-1",
            "display": "2 cups milk",
            "note": "whole milk",
            "checked": false,
            "position": 3,
            "quantity": 2.0,
            "extras": {},
            "unit": null,
            "food": null,
            "groupId": "g",
            "householdId": "h",
            "createdAt": "2026-01-01T00:00:00Z"
        });

        let body = shopping_item_update_body(&item, true);
        let obj = body.as_object().unwrap();
        assert_eq!(obj["checked"], Value::Bool(true));
        assert_eq!(obj["display"], "2 cups milk");
        assert_eq!(obj["note"], "whole milk");
        assert_eq!(obj["position"], 3);
        assert_eq!(obj["quantity"], 2.0);
        assert_eq!(obj["shoppingListId"], "list-1");
        assert!(!obj.contains_key("groupId"));
        assert!(!obj.contains_key("createdAt"));
        assert!(!obj.contains_key("id"));
    }

    #[test]
    fn parses_created_shopping_item_from_create_response() {
        let payload = json!({
            "createdItems": [{
                "quantity": 1.0,
                "unit": null,
                "food": null,
                "referencedRecipe": null,
                "note": "whole milk",
                "display": "whole milk",
                "shoppingListId": "list-1",
                "checked": false,
                "position": 0,
                "extras": {},
                "id": "item-9",
                "groupId": "g",
                "householdId": "h"
            }],
            "updatedItems": [],
            "deletedItems": []
        });

        let item: Value = payload["createdItems"][0].clone();
        let parsed: MealieShoppingListItem = serde_json::from_value(item).unwrap();
        assert_eq!(parsed.id.as_deref(), Some("item-9"));
        assert_eq!(parsed.display.as_deref(), Some("whole milk"));
        assert_eq!(parsed.note.as_deref(), Some("whole milk"));
    }
}
