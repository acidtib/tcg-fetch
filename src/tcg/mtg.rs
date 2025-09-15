use crate::tcg::TcgType;
use crate::utils::files::check_json_files;
use crate::utils::http::{download_json_data, get_user_agent};
use reqwest;
use serde::Deserialize;
use std::io;

#[derive(Debug, Deserialize)]
struct BulkDataItem {
    #[serde(rename = "type")]
    data_type: String,
    download_uri: String,
}

#[derive(Debug, Deserialize)]
struct BulkDataResponse {
    #[serde(default)]
    data: Vec<BulkDataItem>,
}

pub struct MtgApi;

impl MtgApi {
    fn get_api_url() -> &'static str {
        "https://api.scryfall.com/bulk-data"
    }

    fn get_api_type() -> &'static str {
        "mtg_cards"
    }
}

pub async fn fetch_mtg_bulk_data(directory: &str) -> io::Result<Vec<String>> {
    let file_type = MtgApi::get_api_type(); // For file naming
    let scryfall_type = "all_cards"; // For Scryfall API
    let tcg_type = TcgType::Mtg;
    let existing_files = check_json_files(directory, &tcg_type);

    if !existing_files.is_empty() {
        println!("Using existing JSON files");
        return Ok(existing_files);
    }

    println!("Fetching bulk data from Scryfall API...");
    let client = reqwest::Client::new();

    let response = client
        .get(MtgApi::get_api_url())
        .header("User-Agent", get_user_agent())
        .send()
        .await
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to send request: {}", e),
            )
        })?;

    println!("Response status: {}", response.status());

    let response_text = response.text().await.map_err(|e| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to get response text: {}", e),
        )
    })?;

    let bulk_data: BulkDataResponse = serde_json::from_str(&response_text).map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("Failed to parse JSON: {}", e))
    })?;

    let mut downloaded_files = Vec::new();

    for item in bulk_data.data {
        if item.data_type == scryfall_type {
            let file_path = download_json_data(&file_type, &item.download_uri, directory).await?;
            downloaded_files.push(file_path);
            break;
        }
    }

    if downloaded_files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Data type '{}' not found in Scryfall bulk data",
                scryfall_type
            ),
        ));
    }

    Ok(downloaded_files)
}
