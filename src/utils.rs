use std::fs;
use std::path::Path;
use std::io::{self, Write};
use serde::Deserialize;
use tokio;
use futures::future::join_all;
use reqwest;
use indicatif::{ProgressBar, ProgressStyle};

// Map our data types to Scryfall's types
pub const BULK_DATA_TYPES: [&str; 4] = ["unique_artwork", "oracle_cards", "default_cards", "all_cards"];

// Map DataType enum to Scryfall's type strings
pub fn get_scryfall_type(data_type: &super::DataType) -> &'static str {
    match data_type {
        super::DataType::Unique => "unique_artwork",
        super::DataType::Oracle => "oracle_cards",
        super::DataType::Default => "default_cards",
        super::DataType::All => "all_cards",
    }
}

#[derive(Debug, Deserialize)]
struct BulkDataItem {
    #[serde(rename = "type")]
    data_type: String,
    download_uri: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct BulkDataResponse {
    object: String,
    #[serde(default)]
    data: Vec<BulkDataItem>,
}

#[derive(Debug, Deserialize)]
struct ImageUris {
    small: String,
    normal: String,
    large: String,
    png: String,
    art_crop: String,
    border_crop: String,
}

#[derive(Debug, Deserialize)]
struct Card {
    id: String,
    image_uris: Option<ImageUris>,
}

pub fn ensure_directories(base_path: &str) -> io::Result<()> {
    let base_path = Path::new(base_path);

    // Create base directory if it doesn't exist
    if !base_path.exists() {
        fs::create_dir_all(&base_path)?;
        println!("Created base directory: {}", base_path.display());
    }

    // Create required subdirectories
    let subdirs = ["data", "data/train", "data/test", "data/valid"];
    for subdir in subdirs {
        let dir_path = base_path.join(subdir);
        if !dir_path.exists() {
            fs::create_dir(&dir_path)?;
            println!("Created directory: {}", dir_path.display());
        }
    }

    println!("All required directories are ready!");
    Ok(())
}

pub fn check_json_files(directory: &str) -> Vec<String> {
    let base_path = Path::new(directory);
    let required_json_files: Vec<String> = BULK_DATA_TYPES
        .iter()
        .map(|data_type| base_path.join(format!("{}.json", data_type)).to_string_lossy().into_owned())
        .collect();

    required_json_files
        .into_iter()
        .filter(|file| Path::new(file).exists())
        .collect()
}

async fn download_json_data(data_type: &str, download_uri: &str, directory: &str) -> io::Result<String> {
    let client = reqwest::Client::new();
    let file_path = Path::new(directory).join(format!("{}.json", data_type));
    
    println!("Downloading {} data...", data_type);
    
    let response = client.get(download_uri)
        .header("User-Agent", "OjoFetchMagic/1.0")
        .send()
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    let bytes = response.bytes()
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    tokio::fs::write(&file_path, &bytes)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to write file: {}", e)))?;

    println!("Successfully downloaded: {}", file_path.display());
    Ok(file_path.to_string_lossy().into_owned())
}

pub async fn fetch_bulk_data(directory: &str, data_type: &super::DataType) -> io::Result<Vec<String>> {
    let target_type = get_scryfall_type(data_type);
    let existing_files = check_json_files(directory);
    
    // Check if we already have the JSON file
    if !existing_files.is_empty() {
        println!("Using existing JSON files");
        return Ok(existing_files);
    }
    
    println!("Fetching bulk data from Scryfall API...");
    let client = reqwest::Client::new();

    let response = client.get("https://api.scryfall.com/bulk-data")
        .header("User-Agent", "OjoFetchMagic/1.0")
        .send()
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Request error: {}", e)))?;

    println!("Response status: {}", response.status());
    
    let response_text = response.text()
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to get response text: {}", e)))?;
    
    let bulk_data: BulkDataResponse = serde_json::from_str(&response_text)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to parse JSON: {}", e)))?;
    
    let mut downloaded_files = Vec::new();
    
    // Find and download the requested data type
    for item in bulk_data.data {
        if item.data_type == target_type {
            let file_path = download_json_data(&target_type, &item.download_uri, directory).await?;
            downloaded_files.push(file_path);
            break;
        }
    }
    
    if downloaded_files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Data type '{}' not found in Scryfall bulk data", target_type),
        ));
    }
    
    Ok(downloaded_files)
}

pub async fn download_card_images(json_path: &str, output_dir: &str, amount: Option<&str>) -> io::Result<()> {
    let client = reqwest::Client::new();
    let images_dir = Path::new(output_dir).join("data/train");
    fs::create_dir_all(&images_dir)?;

    // Read and parse the JSON file
    let json_content = fs::read_to_string(json_path)?;
    let mut cards: Vec<Card> = serde_json::from_str(&json_content)?;
    
    // Handle amount parameter
    if let Some(amt) = amount {
        if amt != "all" {
            if let Ok(limit) = amt.parse::<usize>() {
                cards.truncate(limit);
            } else {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid amount value"));
            }
        }
    }
    
    let total_cards = cards.len();
    println!("Found {} cards in JSON file, downloading {} cards", total_cards, total_cards);

    let pb = ProgressBar::new(total_cards as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
        .unwrap()
        .progress_chars("#>-"));

    let pb_clone = pb.clone();
    
    let downloads = cards.into_iter()
        .filter_map(|card| {
            let image_uris = card.image_uris?;
            let image_path = images_dir.join(format!("{}.png", card.id));
            let client = client.clone();
            let pb = pb_clone.clone();
            
            // Skip if image already exists
            if image_path.exists() {
                pb.inc(1);
                return None;
            }

            Some(async move {
                match client.get(&image_uris.png)
                    .header("User-Agent", "OjoFetchMagic/1.0")
                    .send()
                    .await {
                    Ok(response) => {
                        if let Ok(bytes) = response.bytes().await {
                            if let Ok(_) = tokio::fs::write(&image_path, &bytes).await {
                                pb.inc(1);
                                Ok(())
                            } else {
                                Err(format!("Failed to write image: {}", card.id))
                            }
                        } else {
                            Err(format!("Failed to get bytes for image: {}", card.id))
                        }
                    },
                    Err(e) => {
                        Err(format!("Failed to download image {}: {}", card.id, e))
                    }
                }
            })
        })
        .collect::<Vec<_>>();

    let results = join_all(downloads).await;
    pb.finish_with_message("Download completed");

    let (successes, failures): (Vec<_>, Vec<_>) = results.into_iter()
        .partition(|r| r.is_ok());

    println!("\nSuccessfully downloaded {} card images", successes.len());
    
    if !failures.is_empty() {
        println!("\nFailed to download {} images:", failures.len());
        for error in failures {
            if let Err(e) = error {
                eprintln!("  - {}", e);
            }
        }
    }

    Ok(())
}
