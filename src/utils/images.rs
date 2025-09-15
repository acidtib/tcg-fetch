use crate::tcg::{TcgType, UnifiedCard};
use crate::utils::http::get_user_agent;
use futures::stream::StreamExt;
use image::GenericImageView;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use reqwest;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::{atomic::AtomicUsize, Arc};

/// Validate that an image file is not corrupted and has reasonable dimensions
pub fn validate_image(image_path: &Path) -> io::Result<()> {
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

/// Process an image by resizing it and converting to JPEG format
pub fn process_image(
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

/// Download and process card images from JSON data
pub async fn download_card_images(
    json_path: &str,
    output_dir: &str,
    amount: Option<&str>,
    thread_count: usize,
    width: u32,
    height: u32,
    tcg_type: &TcgType,
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
        // Parse MTG format - need to define temporary struct for deserialization
        #[derive(serde::Deserialize)]
        struct TempMtgCard {
            id: String,
            image_uris: Option<TempImageUris>,
        }

        #[derive(serde::Deserialize)]
        struct TempImageUris {
            png: String,
        }

        let mtg_cards: Vec<TempMtgCard> = serde_json::from_str(&json_content)?;
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
            TcgType::Mtg => ("png", "jpg"),
            TcgType::Ga => ("jpg", "jpg"),
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
                    .header("User-Agent", get_user_agent())
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

/// Batch check which cards already exist to avoid re-downloading
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

// TODO: Add tests with proper test dependencies
