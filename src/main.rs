use clap::ValueEnum;
use clap::{Parser, Subcommand};
use std::thread;
mod augmentation;
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
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Fetch trading card game data from various APIs
    Fetch {
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
    },
    /// Generate augmented versions of TCG card images
    Augment {
        /// Path to the dataset directory (should have train/ subdir)
        #[arg(short, long)]
        path: String,

        /// Number of augmented versions to generate per image
        #[arg(short, long, default_value_t = 5)]
        amount: u32,

        /// Verify image integrity after augmentation
        #[arg(long, default_value_t = false)]
        verify: bool,
    },
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Fetch {
            tcg,
            path,
            amount,
            threads,
            width,
            height,
        } => {
            println!("TCG: {:?}", tcg);
            println!("Path: {}", path);
            println!("Fetching data of type: All");

            // Ensure the output directory exists
            utils::ensure_directories(&path)?;

            // Fetch and download JSON file for the selected data type
            match utils::fetch_bulk_data(&path, &tcg).await {
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
                            &path,
                            amount.as_deref(),
                            threads,
                            width,
                            height,
                            &tcg,
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

                    // Count and display the number of directories in train folder
                    if let Err(e) = utils::count_train_directories(&path) {
                        eprintln!("Error counting train directories: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Error fetching bulk data: {}", e);
                }
            }
        }
        Commands::Augment {
            path,
            amount,
            verify,
        } => {
            let augmentation_args = augmentation::AugmentationArgs {
                path,
                amount,
                verify,
            };

            if let Err(e) = augmentation::augment_dataset(augmentation_args).await {
                eprintln!("Error during augmentation: {}", e);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
