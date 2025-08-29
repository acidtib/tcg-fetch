use futures::stream::StreamExt;
use image::GenericImageView;
use indicatif::{ProgressBar, ProgressStyle};
use rand::seq::SliceRandom;
use rayon::prelude::*;
use reqwest;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{atomic::AtomicUsize, Arc};
use tokio;

// Map our data types to Scryfall's types
pub const BULK_DATA_TYPES: [&str; 4] = [
    "unique_artwork",
    "oracle_cards",
    "default_cards",
    "all_cards",
];

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

#[derive(Debug, Deserialize)]
struct Card {
    id: String,
    image_uris: Option<ImageUris>,
}

pub fn ensure_directories(base_path: &str) -> io::Result<()> {
    let base_path = Path::new(base_path);

    let dirs_to_create = vec![
        base_path.to_path_buf(),
        base_path.join("data"),
        base_path.join("data/train"),
        base_path.join("data/test"),
        base_path.join("data/validation"),
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

pub fn check_json_files(directory: &str) -> Vec<String> {
    let base_path = Path::new(directory);
    let required_json_files: Vec<(String, PathBuf)> = BULK_DATA_TYPES
        .iter()
        .map(|data_type| {
            let path = base_path.join(format!("{}.json", data_type));
            (path.to_string_lossy().into_owned(), path)
        })
        .collect();

    required_json_files
        .into_par_iter()
        .filter(|(_, path)| path.exists())
        .map(|(file_str, _)| file_str)
        .collect()
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
        .header("User-Agent", "OjoFetchMagic/1.0")
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

pub async fn fetch_bulk_data(
    directory: &str,
    data_type: &super::DataType,
) -> io::Result<Vec<String>> {
    let target_type = get_scryfall_type(data_type);
    let existing_files = check_json_files(directory);

    // Check if we already have the JSON file
    if !existing_files.is_empty() {
        println!("Using existing JSON files");
        return Ok(existing_files);
    }

    println!("Fetching bulk data from Scryfall API...");
    let client = reqwest::Client::new();

    let response = client
        .get("https://api.scryfall.com/bulk-data")
        .header("User-Agent", "OjoFetchMagic/1.0")
        .send()
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Request error: {}", e)))?;

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
            format!(
                "Data type '{}' not found in Scryfall bulk data",
                target_type
            ),
        ));
    }

    Ok(downloaded_files)
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
) -> io::Result<(usize, usize)> {
    let client = reqwest::Client::new();
    let images_dir = Path::new(output_dir).join("data/train");
    fs::create_dir_all(&images_dir)?;

    // Read and parse the JSON file
    let json_content = fs::read_to_string(json_path)?;
    let cards: Vec<Card> = serde_json::from_str(&json_content)?;

    // Filter cards that have image URIs first
    let cards_with_images: Vec<_> = cards
        .into_iter()
        .filter(|card| card.image_uris.is_some())
        .collect();

    let total_available = cards_with_images.len();

    // Handle amount parameter
    let mut cards_to_process = cards_with_images;
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
        let image_uris = card.image_uris.unwrap();
        let card_dir = images_dir.join(&card.id);
        let temp_png_path = card_dir.join("temp.png");
        let final_jpg_path = card_dir.join("0000.jpg");
        let client = client.clone();
        let pb = pb_clone.clone();
        let skipped_soon_clone = skipped_soon.clone();

        {
            let temp_path = temp_png_path.clone();
            let final_path = final_jpg_path.clone();
            async move {
                // Create card directory
                if let Err(e) = fs::create_dir_all(final_path.parent().unwrap()) {
                    pb.inc(1);
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("Failed to create card directory: {}", e),
                    ));
                }

                // Skip cards with Scryfall's placeholder "soon.jpg" image
                if image_uris.png.contains("errors.scryfall.com/soon.jpg") {
                    skipped_soon_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    pb.inc(1);
                    return Ok(());
                }

                match client
                    .get(&image_uris.png)
                    .header("User-Agent", "OjoFetchMagic/1.0")
                    .send()
                    .await
                {
                    Ok(response) => match response.bytes().await {
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
                                        e, image_uris.png
                                    ),
                                ));
                            }

                            if let Err(e) = process_image(&temp_path, &final_path, width, height) {
                                eprintln!(
                                    "Error processing image {} -> {}: {}",
                                    temp_path.display(),
                                    final_path.display(),
                                    e
                                );
                                // Only try to cleanup temp file if it still exists (process_image failed)
                                if temp_path.exists() {
                                    if let Err(cleanup_err) = fs::remove_file(&temp_path) {
                                        eprintln!("Failed to cleanup temp file: {}", cleanup_err);
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
                    },
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

    let failed_downloads = results.iter().filter(|r| r.is_err()).count();
    if failed_downloads > 0 {
        eprintln!("Warning: {} downloads failed", failed_downloads);
    }

    let final_skipped_existing = skipped_existing.load(std::sync::atomic::Ordering::Relaxed);
    let final_skipped_soon = skipped_soon.load(std::sync::atomic::Ordering::Relaxed);

    Ok((final_skipped_existing, final_skipped_soon))
}

pub fn split_dataset(base_path: &str) -> io::Result<()> {
    let train_dir = Path::new(base_path).join("data/train");
    let test_dir = Path::new(base_path).join("data/test");
    let valid_dir = Path::new(base_path).join("data/validation");

    // Create directories if they don't exist
    fs::create_dir_all(&test_dir)?;
    fs::create_dir_all(&valid_dir)?;

    // Get all directories in parallel
    let get_dirs_parallel = |dir: &Path| -> io::Result<Vec<PathBuf>> {
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let entries: Vec<_> = fs::read_dir(dir)?.filter_map(|entry| entry.ok()).collect();

        Ok(entries
            .par_iter()
            .filter_map(|entry| {
                let path = entry.path();
                if path.is_dir() {
                    Some(path)
                } else {
                    None
                }
            })
            .collect())
    };

    let mut train_cards = get_dirs_parallel(&train_dir)?;
    let existing_test_cards = get_dirs_parallel(&test_dir)?;
    let existing_valid_cards = get_dirs_parallel(&valid_dir)?;

    let total_cards = train_cards.len() + existing_test_cards.len() + existing_valid_cards.len();

    // Calculate target numbers for test and validation sets
    let target_test_count = (total_cards as f32 * 0.03).ceil() as usize;
    let target_valid_count = (total_cards as f32 * 0.01).ceil() as usize;

    // Calculate how many additional card directories we need
    let needed_test_cards = target_test_count.saturating_sub(existing_test_cards.len());
    let needed_valid_cards = target_valid_count.saturating_sub(existing_valid_cards.len());

    if needed_test_cards == 0 && needed_valid_cards == 0 {
        println!("Test and validation sets already have the correct number of card directories");
        println!(
            "Total cards: {}, Test: {}, Validation: {}",
            total_cards,
            existing_test_cards.len(),
            existing_valid_cards.len()
        );
        return Ok(());
    }

    println!(
        "Moving {} cards to test and {} cards to validation",
        needed_test_cards, needed_valid_cards
    );

    // Shuffle train cards for random selection
    let mut rng = rand::rng();
    train_cards.shuffle(&mut rng);

    // Move cards to test set in parallel if needed
    if needed_test_cards > 0 && train_cards.len() >= needed_test_cards {
        let test_cards: Vec<_> = train_cards.drain(0..needed_test_cards).collect();

        test_cards
            .par_iter()
            .try_for_each(|card_dir| -> io::Result<()> {
                let card_name = card_dir.file_name().unwrap();
                let dest_dir = test_dir.join(card_name);
                copy_card_directory(card_dir, &dest_dir)?;
                fs::remove_dir_all(card_dir)?;
                Ok(())
            })?;
    }

    // Move cards to validation set in parallel if needed
    if needed_valid_cards > 0 && train_cards.len() >= needed_valid_cards {
        let valid_cards: Vec<_> = train_cards.drain(0..needed_valid_cards).collect();

        valid_cards
            .par_iter()
            .try_for_each(|card_dir| -> io::Result<()> {
                let card_name = card_dir.file_name().unwrap();
                let dest_dir = valid_dir.join(card_name);
                copy_card_directory(card_dir, &dest_dir)?;
                fs::remove_dir_all(card_dir)?;
                Ok(())
            })?;
    }

    let final_train = train_cards.len();
    let final_test = existing_test_cards.len() + needed_test_cards;
    let final_valid = existing_valid_cards.len() + needed_valid_cards;

    println!(
        "Dataset split complete: Train: {}, Test: {}, Validation: {}",
        final_train, final_test, final_valid
    );

    Ok(())
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

// Helper function to copy a card directory and all its contents
fn copy_card_directory(src: &Path, dest: &Path) -> io::Result<()> {
    fs::create_dir_all(dest)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if src_path.is_file() {
            fs::copy(&src_path, &dest_path)?;
        }
    }

    Ok(())
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
