use clap::Parser;
use clap::ValueEnum;
use std::thread;
mod utils;

#[derive(Debug, Clone, ValueEnum)]
enum TcgType {
    /// Magic: The Gathering
    Mtg,
    /// Grand Archive
    Ga,
}

/// Simple program to fetch trading card game data from various APIs
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Trading card game type to fetch data for
    #[arg(value_enum)]
    tcg: TcgType,
    /// Path where to save the data
    #[arg(short, long, default_value = "tcg-data")]
    path: String,

    /// Amount of cards to fetch
    #[arg(short, long, default_value = "all")]
    amount: Option<String>,

    /// Number of threads to use for downloading images (defaults to number of CPU cores)
    #[arg(short, long, default_value_t = thread::available_parallelism().map_or(1, |p| p.get()))]
    threads: usize,

    /// Width for processed images
    #[arg(long, default_value_t = 500)]
    width: u32,

    /// Height for processed images
    #[arg(long, default_value_t = 700)]
    height: u32,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();

    println!("TCG: {:?}", args.tcg);
    println!("Path: {}", args.path);
    println!("Fetching data of type: All");

    // Ensure the output directory exists
    utils::ensure_directories(&args.path)?;

    // Fetch and download JSON file for the selected data type
    match utils::fetch_bulk_data(&args.path, &args.tcg).await {
        Ok(files) => {
            println!("\nDownloaded JSON files:");
            for file in &files {
                println!("  - {}", file);
            }

            let mut total_skipped_existing = 0;
            let mut total_skipped_soon = 0;
            for file in files {
                println!("\nProcessing file: {}", file);
                match utils::download_card_images(
                    &file,
                    &args.path,
                    args.amount.as_deref(),
                    args.threads,
                    args.width,
                    args.height,
                    &args.tcg,
                )
                .await
                {
                    Ok((skipped_existing, skipped_soon)) => {
                        total_skipped_existing += skipped_existing;
                        total_skipped_soon += skipped_soon;
                    }
                    Err(e) => eprintln!("Error downloading images: {}", e),
                }
            }

            if total_skipped_existing > 0 || total_skipped_soon > 0 {
                println!();
                if total_skipped_existing > 0 {
                    println!("Skipped {} cards (already existed)", total_skipped_existing);
                }
                if total_skipped_soon > 0 {
                    println!(
                        "Skipped {} cards (soon.jpg placeholder images)",
                        total_skipped_soon
                    );
                }
            }

            // Split dataset into test and validation sets
            println!("\nSplitting dataset...");
            if let Err(e) = utils::split_dataset(&args.path) {
                eprintln!("Error splitting dataset: {}", e);
            }

            // Count and display the number of directories in train folder
            if let Err(e) = utils::count_train_directories(&args.path) {
                eprintln!("Error counting train directories: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Error fetching bulk data: {}", e);
        }
    }

    Ok(())
}
