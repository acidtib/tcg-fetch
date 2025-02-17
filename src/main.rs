use clap::Parser;
use clap::ValueEnum;
use std::thread;
use std::fs;
use std::path::Path;
use indicatif::{ProgressBar, ProgressStyle};
mod utils;
mod augment;

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

    /// Amount of cards to fetch
    #[arg(short, long, default_value = "all")]
    amount: Option<String>,

    /// Number of threads to use for downloading images (defaults to number of CPU cores)
    #[arg(short, long, default_value_t = thread::available_parallelism().map_or(1, |p| p.get()))]
    threads: usize,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();

    println!("Path: {}", args.path);
    println!("Fetching data of type: {:?}", args.data);

    // Ensure the output directory exists
    utils::ensure_directories(&args.path)?;

    // Fetch and download JSON file for the selected data type
    match utils::fetch_bulk_data(&args.path, &args.data).await {
        Ok(files) => {
            println!("\nDownloaded JSON files:");
            for file in &files {
                println!("  - {}", file);
            }

            for file in files {
                println!("\nProcessing file: {}", file);
                if let Err(e) = utils::download_card_images(&file, &args.path, args.amount.as_deref(), args.threads).await {
                    eprintln!("Error downloading images: {}", e);
                }
            }

            // // After downloading all images, generate augmented versions
            // println!("\nGenerating augmented images...");
            // let train_dir = Path::new(&args.path).join("data/train");
            
            // // Get all card directories
            // if let Ok(entries) = fs::read_dir(&train_dir) {
            //     let total_dirs = entries.count();
            //     let pb = ProgressBar::new(total_dirs as u64);
            //     pb.set_style(ProgressStyle::default_bar()
            //         .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            //         .unwrap()
            //         .progress_chars("#>-"));

            //     // Process each card directory
            //     if let Ok(entries) = fs::read_dir(&train_dir) {
            //         for entry in entries {
            //             if let Ok(entry) = entry {
            //                 let path = entry.path();
            //                 if path.is_dir() {
            //                     // Find the original image (0000.jpg)
            //                     let original_img = path.join("0000.jpg");
            //                     if original_img.exists() {
            //                         if let Err(e) = augment::generate_augmented_images(&original_img, &path, Some(5)) {
            //                             eprintln!("Error generating augmented images for {}: {}", path.display(), e);
            //                         }
            //                     }
            //                 }
            //                 pb.inc(1);
            //             }
            //         }
            //     }
            //     pb.finish_with_message("Augmentation completed");
            // }

            // Split dataset into test and validation sets
            println!("\nSplitting dataset...");
            if let Err(e) = utils::split_dataset(&args.path) {
                eprintln!("Error splitting dataset: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Error fetching bulk data: {}", e);
        }
    }

    Ok(())
}