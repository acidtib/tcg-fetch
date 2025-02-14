use clap::Parser;
use clap::ValueEnum;
use std::path::Path;
mod utils;

#[derive(Debug, Clone, ValueEnum)]
enum DataType {
    /// Unique cards
    Unique,
    /// Oracle cards
    Oracle,
    /// Default cards
    Default,
    /// All cards
    All,
}

/// Simple program to fetch Magic: The Gathering card data from the Scryfall API
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path where to save the data
    #[arg(short, long, default_value = "magic-data")]
    path: String,

    /// Type of card data to fetch (unique, oracle, default, all)
    #[arg(short, long, value_enum, default_value_t = DataType::Default)]
    data: DataType,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    println!("Path: {}", args.path);
    println!("Fetching data of type: {:?}", args.data);

    // Ensure the output directory exists
    utils::ensure_directories(&args.path)?;

    // Fetch and download JSON file for the selected data type
    match utils::fetch_bulk_data(&args.path, &args.data) {
        Ok(files) => {
            println!("\nDownloaded JSON files:");
            for file in files {
                println!("  - {}", file);
            }
        }
        Err(e) => eprintln!("Error fetching bulk data: {}", e),
    }

    // Download images in the json file
    let data_type = utils::get_scryfall_type(&args.data);
    let json_path = Path::new(&args.path).join(format!("{}.json", data_type));
    
    if json_path.exists() {
        println!("\nDownloading card images...");
        match utils::download_card_images(json_path.to_str().unwrap(), &args.path) {
            Ok(_) => println!("Image download completed successfully!"),
            Err(e) => eprintln!("Error downloading images: {}", e),
        }
    } else {
        eprintln!("JSON file not found: {}", json_path.display());
    }

    Ok(())
}