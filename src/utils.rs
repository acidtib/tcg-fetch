use futures::stream::StreamExt;
use image::{ImageBuffer, RgbImage};
use indicatif::{ProgressBar, ProgressStyle};
use rand::seq::SliceRandom;
use reqwest;
use serde::Deserialize;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
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

    // Create base directory if it doesn't exist
    if !base_path.exists() {
        fs::create_dir_all(&base_path)?;
        println!("Created base directory: {}", base_path.display());
    }

    // Create required subdirectories
    let subdirs = ["data", "data/train", "data/test", "data/validation"];
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
        .map(|data_type| {
            base_path
                .join(format!("{}.json", data_type))
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    required_json_files
        .into_iter()
        .filter(|file| Path::new(file).exists())
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

fn process_image(image_path: &Path, width: u32, height: u32) -> io::Result<()> {
    // Open and decode the image
    let img = image::open(image_path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    // Convert to RGB
    let img = img.into_rgb8();

    // Create a new black background image
    let new_size = (width, height);
    let mut new_img: RgbImage = ImageBuffer::new(new_size.0, new_size.1);

    // Calculate the scaling factor to maintain aspect ratio
    let scale = f32::min(
        new_size.0 as f32 / img.width() as f32,
        new_size.1 as f32 / img.height() as f32,
    );

    // Calculate new dimensions
    let new_width = (img.width() as f32 * scale) as u32;
    let new_height = (img.height() as f32 * scale) as u32;

    // Resize the image using Lanczos3 filter
    let resized = image::imageops::resize(
        &img,
        new_width,
        new_height,
        image::imageops::FilterType::Lanczos3,
    );

    // Calculate position to paste (center)
    let x = ((new_size.0 - new_width) / 2) as i64;
    let y = ((new_size.1 - new_height) / 2) as i64;

    // Copy the resized image onto the black background
    image::imageops::replace(&mut new_img, &resized, x, y);

    // Create the new file path with .jpg extension
    let new_image_path = image_path.with_extension("jpg");

    // Save the processed image as JPEG with high quality
    new_img
        .save_with_format(&new_image_path, image::ImageFormat::Jpeg)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    // Delete the original png image
    fs::remove_file(image_path)?;

    Ok(())
}

pub async fn download_card_images(
    json_path: &str,
    output_dir: &str,
    amount: Option<&str>,
    thread_count: usize,
    width: u32,
    height: u32,
) -> io::Result<()> {
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

    let pb = ProgressBar::new(total_cards as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
            )
            .unwrap()
            .progress_chars("#>-"),
    );

    let pb_clone = pb.clone();

    let downloads = cards_to_process
        .into_iter()
        .map(|card| {
            let image_uris = card.image_uris.unwrap();
            let extension = image_uris
                .png
                .split('?')
                .next()
                .and_then(|url| url.rsplit('.').next())
                .unwrap_or("jpg");

            // Create a subdirectory for each card
            let card_dir = images_dir.join(&card.id);
            let image_path = card_dir.join(format!("0000.{}", extension));
            let client = client.clone();
            let pb = pb_clone.clone();

            {
                let value = card_dir.clone();
                async move {
                    // Skip if image already exists
                    let image_path_jpg = value.join("0000.jpg");
                    if image_path_jpg.exists() {
                        pb.inc(1);
                        return Ok(());
                    }

                    // Create the card's directory if it doesn't exist
                    fs::create_dir_all(&value)?;

                    match client
                        .get(&image_uris.png)
                        .header("User-Agent", "OjoFetchMagic/1.0")
                        .send()
                        .await
                    {
                        Ok(response) => {
                            match response.bytes().await {
                                Ok(bytes) => {
                                    let mut file = fs::File::create(&image_path)?;
                                    file.write_all(&bytes)?;

                                    // Process the downloaded image
                                    if let Err(e) = process_image(&image_path, width, height) {
                                        eprintln!(
                                            "Error processing image {}: {}",
                                            image_path.display(),
                                            e
                                        );
                                    }

                                    pb.inc(1);
                                    Ok(())
                                }
                                Err(e) => Err(io::Error::new(io::ErrorKind::Other, e.to_string())),
                            }
                        }
                        Err(e) => Err(io::Error::new(io::ErrorKind::Other, e.to_string())),
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    // Process downloads in parallel with the specified number of threads
    let stream = futures::stream::iter(downloads)
        .buffer_unordered(thread_count)
        .collect::<Vec<_>>();

    let results = stream.await;
    pb.finish_with_message("Download completed");

    // Count successes and failures
    let failures: Vec<_> = results.into_iter().filter(|r| r.is_err()).collect();

    if !failures.is_empty() {
        println!("\nFailed to download {} images", failures.len());
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to download {} images", failures.len()),
        ));
    }

    Ok(())
}

pub fn split_dataset(base_path: &str) -> io::Result<()> {
    let train_dir = Path::new(base_path).join("data/train");
    let test_dir = Path::new(base_path).join("data/test");
    let valid_dir = Path::new(base_path).join("data/validation");

    // Create directories if they don't exist
    fs::create_dir_all(&test_dir)?;
    fs::create_dir_all(&valid_dir)?;

    // Get all jpg files from train directory
    let mut train_files: Vec<_> = fs::read_dir(&train_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_dir() {
                // Look for 0000.jpg in each card directory
                let jpg_path = path.join("0000.jpg");
                if jpg_path.exists() {
                    Some(jpg_path)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    // Get existing test and validation files
    let existing_test_files: Vec<_> = fs::read_dir(&test_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_dir() {
                let jpg_path = path.join("0000.jpg");
                if jpg_path.exists() {
                    Some(jpg_path)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    let existing_valid_files: Vec<_> = fs::read_dir(&valid_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_dir() {
                let jpg_path = path.join("0000.jpg");
                if jpg_path.exists() {
                    Some(jpg_path)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    let total_images = train_files.len() + existing_test_files.len() + existing_valid_files.len();

    // Calculate target numbers for test and validation sets
    let target_test_count = (total_images as f32 * 0.03).ceil() as usize;
    let target_valid_count = (total_images as f32 * 0.01).ceil() as usize;

    // Calculate how many additional files we need
    let needed_test_files = target_test_count.saturating_sub(existing_test_files.len());
    let needed_valid_files = target_valid_count.saturating_sub(existing_valid_files.len());

    if needed_test_files == 0 && needed_valid_files == 0 {
        println!("Test and validation sets already have the correct number of images");
        println!(
            "Total images: {}, Test: {}, Validation: {}",
            total_images,
            existing_test_files.len(),
            existing_valid_files.len()
        );
        return Ok(());
    }

    // Randomly shuffle the train files
    let mut rng = rand::rng();
    train_files.shuffle(&mut rng);

    // Copy needed files to test set
    if needed_test_files > 0 {
        let test_images = &train_files[..needed_test_files];
        for src_path in test_images {
            let card_id = src_path.parent().unwrap().file_name().unwrap();
            let dest_dir = test_dir.join(card_id);
            fs::create_dir_all(&dest_dir)?;
            let dest_path = dest_dir.join("0000.jpg");
            fs::copy(src_path, &dest_path)?;
        }
    }

    // Copy needed files to validation set
    if needed_valid_files > 0 {
        let start = needed_test_files;
        let end = start + needed_valid_files;
        let valid_images = &train_files[start..end.min(train_files.len())];
        for src_path in valid_images {
            let card_id = src_path.parent().unwrap().file_name().unwrap();
            let dest_dir = valid_dir.join(card_id);
            fs::create_dir_all(&dest_dir)?;
            let dest_path = dest_dir.join("0000.jpg");
            fs::copy(src_path, &dest_path)?;
        }
    }

    println!("Dataset split updated:");
    println!("Total images: {}", total_images);
    println!(
        "Test set: {} existing + {} new = {} total (target: {})",
        existing_test_files.len(),
        needed_test_files,
        existing_test_files.len() + needed_test_files,
        target_test_count
    );
    println!(
        "Validation set: {} existing + {} new = {} total (target: {})",
        existing_valid_files.len(),
        needed_valid_files,
        existing_valid_files.len() + needed_valid_files,
        target_valid_count
    );

    Ok(())
}
