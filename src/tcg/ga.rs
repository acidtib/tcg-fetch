use crate::tcg::TcgType;
use crate::utils::files::check_json_files;
use crate::utils::http::get_user_agent;
use futures::stream::StreamExt;
use reqwest;
use serde::Deserialize;
use serde_json;
use std::io;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct GaCard {
    #[allow(dead_code)]
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Deserialize)]
pub struct GaCardDetail {
    #[allow(dead_code)]
    pub name: String,
    pub editions: Vec<GaEdition>,
}

#[derive(Debug, Deserialize)]
pub struct GaEdition {
    pub slug: String,
    pub image: String,
}

pub struct GaApi;

impl GaApi {
    fn get_api_url() -> &'static str {
        "https://api.gatcg.com/cards/all"
    }
}

async fn fetch_ga_card_detail(client: &reqwest::Client, slug: &str) -> io::Result<GaCardDetail> {
    let url = format!("https://api.gatcg.com/cards/{}", slug);
    let response = client
        .get(&url)
        .header("User-Agent", get_user_agent())
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Request error: {}", e)))?;

    let card_detail: GaCardDetail = response.json().await.map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("Failed to parse JSON: {}", e))
    })?;

    Ok(card_detail)
}

pub async fn fetch_ga_all_cards(directory: &str) -> io::Result<Vec<String>> {
    let tcg_type = TcgType::Ga;
    let existing_files = check_json_files(directory, &tcg_type);

    if !existing_files.is_empty() {
        println!("Using existing JSON files");
        return Ok(existing_files);
    }

    println!("Fetching GA card data from API...");
    let client = reqwest::Client::new();

    // First, get all card names and slugs
    let response = client
        .get(GaApi::get_api_url())
        .header("User-Agent", get_user_agent())
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Request error: {}", e)))?;

    let cards: Vec<GaCard> = response.json().await.map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("Failed to parse JSON: {}", e))
    })?;

    println!(
        "Found {} cards, fetching detailed information...",
        cards.len()
    );

    // Create a temporary file to store all the card data
    let temp_file = Path::new(directory).join("ga_cards.json");
    let mut all_cards_data = Vec::new();

    // Use parallel processing to fetch card details
    let card_details = futures::stream::iter(cards.into_iter().map(|card| {
        let client = &client;
        async move {
            match fetch_ga_card_detail(client, &card.slug).await {
                Ok(detail) => Some(detail),
                Err(e) => {
                    eprintln!("Failed to fetch details for {}: {}", card.slug, e);
                    None
                }
            }
        }
    }))
    .buffer_unordered(10) // Process 10 cards concurrently
    .collect::<Vec<_>>()
    .await;

    // Collect all edition data - one entry per edition
    for card_detail in card_details.into_iter().flatten() {
        for edition in card_detail.editions {
            all_cards_data.push(serde_json::json!({
                "slug": edition.slug,
                "image": format!("https://api.gatcg.com{}", edition.image)
            }));
        }
    }

    // Write the collected data to a JSON file
    let json_data = serde_json::to_string_pretty(&all_cards_data).map_err(|e| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to serialize JSON: {}", e),
        )
    })?;

    std::fs::write(&temp_file, json_data)?;
    println!("Successfully downloaded: {}", temp_file.display());

    Ok(vec![temp_file.to_string_lossy().into_owned()])
}
