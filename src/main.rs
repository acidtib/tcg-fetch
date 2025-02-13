use clap::Parser;
mod utils;

/// Simple program to fetch Magic: The Gathering card data from the Scryfall API
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path where to save the data
    #[arg(short, long, default_value = "magic-data")]
    path: String,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    // Ensure the output directory exists
    utils::ensure_directories(&args.path)?;
    
    Ok(())
}