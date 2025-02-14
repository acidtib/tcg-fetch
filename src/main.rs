use clap::Parser;
use clap::ValueEnum;
use std::thread;
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

    /// Amount of cards to fetch
    #[arg(short, long, default_value = "all")]
    amount: Option<String>,

    /// Number of threads to use for downloading images (defaults to number of CPU cores)
    #[arg(short = 't', long, default_value_t = thread::available_parallelism().map_or(1, |p| p.get()))]
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

            // Download card images
            for file in files {
                if let Err(e) = utils::download_card_images(&file, &args.path, args.amount.as_deref(), args.threads).await {
                    eprintln!("Error downloading images: {}", e);
                    return Err(e);
                }
            }
        }
        Err(e) => eprintln!("Error fetching bulk data: {}", e),
    }

    Ok(())
}