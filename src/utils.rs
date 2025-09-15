use futures::stream::StreamExt;
use image::GenericImageView;
use indicatif::{ProgressBar, ProgressStyle};

use rayon::prelude::*;
use reqwest;
use serde::Deserialize;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{atomic::AtomicUsize, Arc};
use tokio;

// Get API type for TCG
pub fn get_api_type(tcg_type: &super::TcgType) -> &'static str {
    match tcg_type {
        super::TcgType::Mtg => "mtg_cards",
        super::TcgType::Ga => "ga_cards",
    }
}

// Get API URL based on TCG type
pub fn get_api_url(tcg_type: &super::TcgType) -> &'static str {
    match tcg_type {
        super::TcgType::Mtg => "https://api.scryfall.com/bulk-data",
        super::TcgType::Ga => "https://api.gatcg.com/cards/all",
    }
}

// Get user agent string based on TCG type
pub fn get_user_agent() -> &'static str {
    "TCGFetch"
}

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

#[derive(Debug, Deserialize)]
struct ImageUris {
    png: String,
}

// Grand Archive API structs
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

#[derive(Debug, Deserialize)]
struct Card {
    id: String,
    image_uris: Option<ImageUris>,
}

// Unified card structure for both MTG and GA
#[derive(Debug, Clone)]
struct UnifiedCard {
    id: String,
    image_url: String,
}

pub fn ensure_directories(base_path: &str) -> io::Result<()> {
    let base_path = Path::new(base_path);

    let dirs_to_create = vec![
        base_path.to_path_buf(),
        base_path.join("data"),
        base_path.join("data/train"),
    ];

    // Check which directories don't exist
    let missing_dirs: Vec<PathBuf> = dirs_to_create
        .into_par_iter()
        .filter(|dir| !dir.exists())
        .collect();

    // Create missing directories in parallel
    missing_dirs
        .par_iter()
        .try_for_each(|dir| -> io::Result<()> {
            fs::create_dir_all(dir)?;
            println!("Created directory: {}", dir.display());
            Ok(())
        })?;

    println!("All required directories are ready!");
    Ok(())
}

pub fn check_json_files(directory: &str, tcg_type: &super::TcgType) -> Vec<String> {
    let base_path = Path::new(directory);
    let mut existing_files = Vec::new();

    // Check for the specific TCG type file
    let file_path = base_path.join(format!("{}.json", get_api_type(tcg_type)));
    if file_path.exists() {
        existing_files.push(file_path.to_string_lossy().into_owned());
    }

    existing_files
}

async fn download_json_data(
    data_type: &str,
    download_uri: &str,
    directory: &str,
) -> io::Result<String> {
    let client = reqwest::Client::new();
    let file_path = Path::new(directory).join(format!("{}.json", data_type));

    println!("Downloading {} data...", data_type);

    let response = client
        .get(download_uri)
        .header("User-Agent", "TCGFetch/1.0")
        .send()
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    let bytes = response
        .bytes()
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    tokio::fs::write(&file_path, &bytes).await.map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("Failed to write file: {}", e))
    })?;

    println!("Successfully downloaded: {}", file_path.display());
    Ok(file_path.to_string_lossy().into_owned())
}

// GA-specific functions
async fn fetch_ga_card_detail(client: &reqwest::Client, slug: &str) -> io::Result<GaCardDetail> {
    let url = format!("https://api.gatcg.com/cards/{}", slug);
    let response = client
        .get(&url)
        .header("User-Agent", "TCGFetch-GA/1.0")
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Request error: {}", e)))?;

    let card_detail: GaCardDetail = response.json().await.map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("Failed to parse JSON: {}", e))
    })?;

    Ok(card_detail)
}

async fn fetch_ga_all_cards(directory: &str) -> io::Result<Vec<String>> {
    println!("Fetching GA card data from API...");
    let client = reqwest::Client::new();

    // First, get all card names and slugs
    let response = client
        .get("https://api.gatcg.com/cards/all")
        .header("User-Agent", "TCGFetch-GA/1.0")
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

pub async fn fetch_bulk_data(
    directory: &str,
    tcg_type: &super::TcgType,
) -> io::Result<Vec<String>> {
    match tcg_type {
        super::TcgType::Mtg => {
            // Existing MTG logic
            let file_type = get_api_type(tcg_type); // For file naming
            let scryfall_type = "all_cards"; // For Scryfall API
            let existing_files = check_json_files(directory, tcg_type);

            if !existing_files.is_empty() {
                println!("Using existing JSON files");
                return Ok(existing_files);
            }

            println!("Fetching bulk data from Scryfall API...");
            let client = reqwest::Client::new();

            let response = client
                .get(get_api_url(tcg_type))
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

            let bulk_data: BulkDataResponse =
                serde_json::from_str(&response_text).map_err(|e| {
                    io::Error::new(io::ErrorKind::Other, format!("Failed to parse JSON: {}", e))
                })?;

            let mut downloaded_files = Vec::new();

            for item in bulk_data.data {
                if item.data_type == scryfall_type {
                    let file_path =
                        download_json_data(&file_type, &item.download_uri, directory).await?;
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
        super::TcgType::Ga => {
            // GA-specific logic
            let existing_files = check_json_files(directory, tcg_type);

            if !existing_files.is_empty() {
                println!("Using existing JSON files");
                return Ok(existing_files);
            }

            fetch_ga_all_cards(directory).await
        }
    }
}

/// Validates that an image file is not corrupted by attempting to decode it
/// Also checks file size and format validity
///
/// This function performs multiple validation checks:
/// 1. File size validation (minimum 100 bytes, maximum 50MB)
/// 2. Image decoding validation using the `image` crate
/// 3. Dimension validation (minimum 10x10, maximum 10000x10000)
///
/// # Arguments
/// * `image_path` - Path to the image file to validate
///
/// # Returns
/// * `Ok(())` if the image is valid and not corrupted
/// * `Err(io::Error)` if the image is corrupted or invalid
///
/// # Examples
/// ```
/// use std::path::Path;
///
/// // This would validate a downloaded image
/// if let Err(e) = validate_image(Path::new("downloaded_image.jpg")) {
///     eprintln!("Image is corrupted: {}", e);
///     // Clean up the corrupted file...
/// }
/// ```
fn validate_image(image_path: &Path) -> io::Result<()> {
    // Check if file exists and has reasonable size
    let metadata = fs::metadata(image_path)?;
    let file_size = metadata.len();

    // Check for minimum and maximum reasonable file sizes
    if file_size < 100 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Image file too small, likely corrupted",
        ));
    }

    if file_size > 50_000_000 {
        // 50MB limit
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Image file too large, possibly corrupted or invalid",
        ));
    }

    // Attempt to decode the image to check for corruption
    match image::open(image_path) {
        Ok(img) => {
            // Additional validation: check image dimensions
            let (width, height) = img.dimensions();
            if width == 0 || height == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Image has invalid dimensions (0x0)",
                ));
            }

            // Check for reasonable image dimensions (not too small, not absurdly large)
            if width < 10 || height < 10 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Image dimensions too small, likely corrupted",
                ));
            }

            if width > 10000 || height > 10000 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Image dimensions unreasonably large",
                ));
            }

            Ok(())
        }
        Err(e) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Image validation failed: {}", e),
        )),
    }
}

fn process_image(
    source_path: &Path,
    target_path: &Path,
    width: u32,
    height: u32,
) -> io::Result<()> {
    // Open and decode the source image (PNG)
    let img = image::open(source_path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    // Convert to RGB
    let img = img.into_rgb8();

    // Resize the image directly to target dimensions using Lanczos3 filter
    let resized =
        image::imageops::resize(&img, width, height, image::imageops::FilterType::Lanczos3);

    // Save the processed image as JPEG with high quality
    resized
        .save_with_format(target_path, image::ImageFormat::Jpeg)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    // Final validation: ensure the processed JPEG is not corrupted
    // This catches any corruption that might have occurred during processing
    validate_image(target_path)?;

    // Delete the temporary PNG file
    fs::remove_file(source_path)?;

    Ok(())
}

pub async fn download_card_images(
    json_path: &str,
    output_dir: &str,
    amount: Option<&str>,
    thread_count: usize,
    width: u32,
    height: u32,
    tcg_type: &super::TcgType,
) -> io::Result<(usize, usize)> {
    let client = reqwest::Client::new();
    let images_dir = Path::new(output_dir).join("data/train");
    fs::create_dir_all(&images_dir)?;

    // Read and parse the JSON file
    let json_content = fs::read_to_string(json_path)?;

    // Try to determine format and create unified cards
    let unified_cards: Vec<UnifiedCard> = if json_path.contains("ga_cards") {
        // Parse GA format
        let ga_cards: Vec<serde_json::Value> = serde_json::from_str(&json_content)?;
        ga_cards
            .into_iter()
            .map(|card| UnifiedCard {
                id: card["slug"].as_str().unwrap_or("unknown").to_string(),
                image_url: card["image"].as_str().unwrap_or("").to_string(),
            })
            .collect()
    } else {
        // Parse MTG format
        let mtg_cards: Vec<Card> = serde_json::from_str(&json_content)?;
        mtg_cards
            .into_iter()
            .filter_map(|card| {
                if let Some(image_uris) = card.image_uris {
                    Some(UnifiedCard {
                        id: card.id,
                        image_url: image_uris.png,
                    })
                } else {
                    None
                }
            })
            .collect()
    };

    let total_available = unified_cards.len();

    // Handle amount parameter
    let mut cards_to_process = unified_cards;
    if let Some(amt) = amount {
        if amt != "all" {
            if let Ok(limit) = amt.parse::<usize>() {
                cards_to_process.truncate(limit);
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Invalid amount value",
                ));
            }
        }
    }

    let total_cards = cards_to_process.len();
    println!(
        "Found {} cards with images, downloading {} cards using {} threads",
        total_available, total_cards, thread_count
    );

    // Batch check which cards already exist
    let card_ids: Vec<String> = cards_to_process
        .iter()
        .map(|card| card.id.clone())
        .collect();
    let existing_cards = batch_check_existing_cards(output_dir, &card_ids);

    // Filter out cards that already exist
    let cards_to_download: Vec<_> = cards_to_process
        .into_iter()
        .filter(|card| !existing_cards.get(&card.id).unwrap_or(&false))
        .collect();

    let cards_to_download_count = cards_to_download.len();
    let already_existed = total_cards - cards_to_download_count;

    println!("Skipping {} cards that already exist", already_existed);
    println!("Downloading {} new cards", cards_to_download_count);

    if cards_to_download.is_empty() {
        return Ok((already_existed, 0));
    }

    let pb = ProgressBar::new(cards_to_download_count as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
            )
            .unwrap()
            .progress_chars("#>-"),
    );

    let pb_clone = pb.clone();
    let skipped_existing = Arc::new(AtomicUsize::new(already_existed));
    let skipped_soon = Arc::new(AtomicUsize::new(0));

    let downloads = cards_to_download.into_iter().map(|card| {
        let card_dir = images_dir.join(&card.id);
        let (temp_ext, final_ext) = match tcg_type {
            super::TcgType::Mtg => ("png", "jpg"),
            super::TcgType::Ga => ("jpg", "jpg"),
        };
        let temp_file_path = card_dir.join(format!("temp.{}", temp_ext));
        let final_file_path = card_dir.join(format!("0000.{}", final_ext));
        let client = client.clone();
        let pb = pb_clone.clone();
        let skipped_soon_clone = skipped_soon.clone();
        let image_url = card.image_url.clone();

        {
            let temp_path = temp_file_path.clone();
            let final_path = final_file_path.clone();
            async move {
                // Create card directory
                if let Err(e) = fs::create_dir_all(final_path.parent().unwrap()) {
                    pb.inc(1);
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("Failed to create card directory: {}", e),
                    ));
                }

                // Skip cards with placeholder "soon.jpg" image (MTG specific)
                if image_url.contains("errors.scryfall.com/soon.jpg") {
                    skipped_soon_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    pb.inc(1);
                    return Ok(());
                }

                match client
                    .get(&image_url)
                    .header("User-Agent", "TCGFetch/1.0")
                    .send()
                    .await
                {
                    Ok(response) => {
                        if !response.status().is_success() {
                            pb.inc(1);
                            return Err(io::Error::new(
                                io::ErrorKind::Other,
                                format!("HTTP {} for URL: {}", response.status(), image_url),
                            ));
                        }

                        match response.bytes().await {
                            Ok(bytes) => {
                                let mut file = fs::File::create(&temp_path)?;
                                file.write_all(&bytes)?;

                                if let Err(e) = validate_image(&temp_path) {
                                    if let Err(cleanup_err) = fs::remove_file(&temp_path) {
                                        eprintln!(
                                            "Failed to cleanup corrupted image file: {}",
                                            cleanup_err
                                        );
                                    }
                                    pb.inc(1);
                                    return Err(io::Error::new(
                                        io::ErrorKind::InvalidData,
                                        format!(
                                            "Corrupted image detected: {} - URL: {}",
                                            e, image_url
                                        ),
                                    ));
                                }
                                if let Err(e) =
                                    process_image(&temp_path, &final_path, width, height)
                                {
                                    // Only try to cleanup temp file if it still exists (process_image failed)
                                    if temp_path.exists() {
                                        if let Err(cleanup_err) = fs::remove_file(&temp_path) {
                                            eprintln!(
                                                "Failed to cleanup temp file: {}",
                                                cleanup_err
                                            );
                                        }
                                    }
                                    pb.inc(1);
                                    return Err(e);
                                }

                                pb.inc(1);
                                Ok(())
                            }
                            Err(e) => {
                                pb.inc(1);
                                Err(io::Error::new(
                                    io::ErrorKind::Other,
                                    format!("Failed to read response bytes: {}", e),
                                ))
                            }
                        }
                    }
                    Err(e) => {
                        pb.inc(1);
                        Err(io::Error::new(
                            io::ErrorKind::Other,
                            format!("HTTP request failed: {}", e),
                        ))
                    }
                }
            }
        }
    });

    let semaphore = Arc::new(tokio::sync::Semaphore::new(thread_count));
    let results: Vec<_> = futures::stream::iter(downloads)
        .map(|download| {
            let semaphore = semaphore.clone();
            async move {
                let _permit = semaphore.acquire().await.unwrap();
                download.await
            }
        })
        .buffer_unordered(thread_count)
        .collect()
        .await;

    pb.finish_with_message("Download complete!");

    let failed_downloads = results
        .iter()
        .filter(|r: &&Result<(), std::io::Error>| r.is_err())
        .count();
    if failed_downloads > 0 {
        eprintln!("Warning: {} downloads failed", failed_downloads);
    }

    let final_skipped_existing = skipped_existing.load(std::sync::atomic::Ordering::Relaxed);
    let final_skipped_soon = skipped_soon.load(std::sync::atomic::Ordering::Relaxed);

    Ok((final_skipped_existing, final_skipped_soon))
}

/// Batch check if card directories exist for faster filtering during download
pub fn batch_check_existing_cards(base_path: &str, card_ids: &[String]) -> HashMap<String, bool> {
    let train_dir = Path::new(base_path).join("data/train");

    card_ids
        .par_iter()
        .map(|card_id| {
            let card_dir = train_dir.join(card_id);
            let final_jpg = card_dir.join("0000.jpg");
            (card_id.clone(), final_jpg.exists())
        })
        .collect()
}

pub fn count_train_directories(base_path: &str) -> io::Result<()> {
    let train_dir = Path::new(base_path).join("data/train");

    if !train_dir.exists() {
        println!("Train directory does not exist yet");
        return Ok(());
    }

    let entries: Vec<_> = fs::read_dir(&train_dir)?
        .filter_map(|entry| entry.ok())
        .collect();

    let dir_count = entries
        .par_iter()
        .filter(|entry| entry.path().is_dir())
        .count();

    println!("Total train card directories: {}", dir_count);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_academy_guide_json_parsing() {
        let json_data = r#"{
  "classes": [
    "CLERIC"
  ],
  "cost_memory": null,
  "cost_reserve": 3,
  "created_at": "2024-01-24T12:00:00.000Z",
  "durability": null,
  "editions": [
    {
      "card_id": "kk39i1f0ht",
      "collector_number": "042",
      "configuration": "default",
      "created_at": "2024-01-26T12:00:00+00:00",
      "effect": null,
      "effect_raw": null,
      "flavor": "",
      "illustrator": "Leviathan",
      "image": "/cards/images/academy-guide-alc.jpg",
      "last_update": "2025-01-19T12:25:21.173+00:00",
      "orientation": null,
      "rarity": 4,
      "slug": "academy-guide-alc",
      "uuid": "2l8lbewemh",
      "collaborators": [],
      "circulationTemplates": [],
      "circulations": [],
      "other_orientations": [],
      "set": {},
      "effect_html": null
    },
    {
      "card_id": "kk39i1f0ht",
      "collector_number": "120",
      "configuration": "default",
      "created_at": "2024-01-24T12:00:00+00:00",
      "effect": null,
      "effect_raw": null,
      "flavor": null,
      "illustrator": "十尾",
      "image": "/cards/images/academy-guide-p24.jpg",
      "last_update": "2025-01-18T17:40:17.152+00:00",
      "orientation": null,
      "rarity": 6,
      "slug": "academy-guide-p24",
      "uuid": "x99w8eraxx",
      "collaborators": [],
      "circulationTemplates": [],
      "circulations": [
        {
          "created_at": "2025-04-14T16:10:46.103185+00:00",
          "edition_id": "x99w8eraxx",
          "foil": true,
          "kind": "FOIL",
          "last_update": "2025-04-14T16:10:46.071+00:00",
          "population": 160,
          "population_operator": "=",
          "printing": false,
          "uuid": "GhMtde7MVh",
          "variants": [
            {
              "uuid": "tqsCgmQQRy",
              "edition_id": "x99w8eraxx",
              "description": "Ascent Christchurch stamp",
              "image": "/cards/images/academy-guide-p24-chch.jpg",
              "population_operator": "=",
              "population": 32,
              "printing": false,
              "kind": "FOIL",
              "created_at": "2025-05-16T18:17:49.608+00:00",
              "last_update": "2025-05-16T18:17:49.608+00:00"
            }
          ]
        }
      ],
      "other_orientations": [],
      "set": {},
      "effect_html": null
    }
  ],
  "effect": "Champion cards you materialize cost 1 less to materialize.",
  "name": "Academy Guide",
  "slug": "academy-guide"
}"#;

        let card_detail: Result<GaCardDetail, _> = serde_json::from_str(json_data);

        match card_detail {
            Ok(card) => {
                println!("Successfully parsed card: {}", card.name);

                println!("Number of editions: {}", card.editions.len());

                for (i, edition) in card.editions.iter().enumerate() {
                    println!(
                        "Edition {}: slug={}, image={}",
                        i + 1,
                        edition.slug,
                        edition.image
                    );
                }

                // Test the specific data you need
                assert_eq!(card.name, "Academy Guide");
                assert_eq!(card.editions.len(), 2);

                // Check first edition
                assert_eq!(card.editions[0].slug, "academy-guide-alc");
                assert_eq!(
                    card.editions[0].image,
                    "/cards/images/academy-guide-alc.jpg"
                );

                // Check second edition
                assert_eq!(card.editions[1].slug, "academy-guide-p24");
                assert_eq!(
                    card.editions[1].image,
                    "/cards/images/academy-guide-p24.jpg"
                );

                println!("All assertions passed!");
            }
            Err(e) => {
                println!("Failed to parse JSON: {}", e);
                panic!("JSON parsing failed");
            }
        }
    }

    #[tokio::test]
    async fn test_real_academy_guide_api_call() {
        let client = reqwest::Client::new();

        match fetch_ga_card_detail(&client, "academy-guide").await {
            Ok(card_detail) => {
                println!("Successfully fetched card: {}", card_detail.name);
                println!("Number of editions: {}", card_detail.editions.len());

                for (i, edition) in card_detail.editions.iter().enumerate() {
                    println!(
                        "Edition {}: slug={}, image={}",
                        i + 1,
                        edition.slug,
                        edition.image
                    );
                }

                // Basic assertions
                assert_eq!(card_detail.name, "Academy Guide");
                assert!(
                    !card_detail.editions.is_empty(),
                    "Should have at least one edition"
                );

                println!("Real API test passed!");
            }
            Err(e) => {
                println!("API call failed: {}", e);
                // Don't panic for network issues in tests
                println!("This test requires internet connection to GA API");
            }
        }
    }
}
