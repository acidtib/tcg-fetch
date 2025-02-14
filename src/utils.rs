use std::fs;
use std::path::Path;
use std::io::{self, Write, Read};
use serde::Deserialize;
use reqwest::blocking::Client;

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

fn download_json_data(data_type: &str, download_uri: &str, directory: &str) -> io::Result<String> {
    let client = Client::new();
    let file_path = Path::new(directory).join(format!("{}.json", data_type));
    
    println!("Downloading {} data...", data_type);
    
    let mut response = client.get(download_uri)
        .header("User-Agent", "OjoFetchMagic/1.0")
        .send()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded = 0;
    let mut file = fs::File::create(&file_path)?;
    let mut buffer = vec![0; 8192]; // 8KB buffer
    
    loop {
        let bytes_read = response.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        downloaded += bytes_read as u64;
        if total_size > 0 {
            print!("\rDownloading... {:.1}%", (downloaded as f64 / total_size as f64) * 100.0);
        } else {
            print!("\rDownloaded: {} bytes", downloaded);
        }
        io::stdout().flush().ok();
        file.write_all(&buffer[..bytes_read])?;
    }
    println!("\nSuccessfully downloaded: {}", file_path.display());
    
    Ok(file_path.to_string_lossy().into_owned())
}

pub fn fetch_bulk_data(directory: &str, data_type: &super::DataType) -> io::Result<Vec<String>> {
    let target_type = get_scryfall_type(data_type);
    let existing_files = check_json_files(directory);
    
    // Check if the file we want already exists
    if let Some(existing_file) = existing_files.iter().find(|file| {
        file.contains(&format!("{}.json", target_type))
    }) {
        println!("JSON file already exists: {}", existing_file);
        return Ok(vec![existing_file.clone()]);
    }
    
    println!("Fetching bulk data from Scryfall API...");
    let client = Client::new();
    let response = client.get("https://api.scryfall.com/bulk-data")
        .header("User-Agent", "OjoFetchMagic/1.0")
        .send()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Request error: {}", e)))?;

    println!("Response status: {}", response.status());
    
    // Get the response text for debugging
    let response_text = response.text()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to get response text: {}", e)))?;
    
    // Parse the JSON
    let bulk_data: BulkDataResponse = serde_json::from_str(&response_text)
        .map_err(|e| {
            eprintln!("Response body: {}", &response_text);
            io::Error::new(io::ErrorKind::Other, format!("JSON parse error: {}", e))
        })?;
        
    if bulk_data.object != "list" {
        return Err(io::Error::new(io::ErrorKind::Other, "Failed to fetch bulk data: unexpected response format"));
    }
    
    let mut downloaded_files = Vec::new();
    
    // Find and download only the requested data type
    if let Some(item) = bulk_data.data.into_iter().find(|item| item.data_type == target_type) {
        println!("Found bulk data item: {} ({})", item.name, item.data_type);
        match download_json_data(&item.data_type, &item.download_uri, directory) {
            Ok(file) => downloaded_files.push(file),
            Err(e) => eprintln!("Failed to download {}: {}", item.data_type, e),
        }
    } else {
        return Err(io::Error::new(io::ErrorKind::Other, format!("Bulk data type '{}' not found", target_type)));
    }
    
    Ok(downloaded_files)
}

pub fn download_card_images(json_path: &str, output_dir: &str) -> io::Result<()> {
    let client = Client::new();
    let images_dir = Path::new(output_dir).join("data/train");
    fs::create_dir_all(&images_dir)?;

    // Read and parse the JSON file
    let json_content = fs::read_to_string(json_path)?;
    let cards: Vec<Card> = serde_json::from_str(&json_content)?;
    let total_cards = cards.len();

    println!("Found {} cards in JSON file", total_cards);
    let mut downloaded = 0;

    for card in cards {
        if let Some(image_uris) = card.image_uris {
            let image_path = images_dir.join(format!("{}.png", card.id));
            
            // Skip if image already exists
            if image_path.exists() {
                println!("Image already exists: {}", image_path.display());
                downloaded += 1;
                continue;
            }

            print!("\rDownloading image for card ID: {} ({}/{})", card.id, downloaded + 1, total_cards);
            io::stdout().flush().ok();

            match client.get(&image_uris.png)
                .header("User-Agent", "OjoFetchMagic/1.0")
                .send() {
                Ok(mut response) => {
                    let mut file = fs::File::create(&image_path)?;
                    io::copy(&mut response, &mut file)?;
                    downloaded += 1;
                },
                Err(e) => {
                    eprintln!("\nError downloading image for card {}: {}", card.id, e);
                }
            }
        }
    }

    println!("\nSuccessfully downloaded {} card images", downloaded);
    Ok(())
}
