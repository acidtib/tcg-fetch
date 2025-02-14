# Ojo Fetch Magic

A Rust CLI tool to fetch Magic: The Gathering card data from the Scryfall API. This tool helps organize card data into train, test, and validation sets.

## Features

- Fetches card data from Scryfall API
- Downloads different types of card data (unique artwork, oracle cards, default cards, rulings, or all cards)
- Organizes data into train/test/validation sets
- Command-line interface with customizable output path
- Checks for existing files to avoid unnecessary downloads
- Progress indicator for large file downloads

## Installation

Make sure you have Rust and Cargo installed on your system. Then clone this repository:

```bash
git clone https://github.com/acidtib/ojo-fetch-magic.git
cd ojo-fetch-magic
cargo build --release
```

## Usage

Run the program with default settings (fetches all cards by default):

```bash
cargo run
```

Specify the type of data to fetch using the `--data` or `-d` flag:

```bash
cargo run -- --data unique    # Download unique artwork data
cargo run -- --data oracle    # Download oracle cards data
cargo run -- --data default   # Download default cards data
cargo run -- --data ruling    # Download rulings data
cargo run -- --data all       # Download all cards data
```

Specify a custom output directory:

```bash
cargo run -- --path custom-data-dir
```

Combine both options:

```bash
cargo run -- --data oracle --path custom-data-dir
```

This will create a directory with the following structure:
```
<output-dir>/
├── data/
│   ├── train/
│   ├── test/
│   └── valid/
└── <data-type>.json
```

For help and available options:

```bash
cargo run -- --help
```

## Dependencies

- clap: Command-line argument parsing
- reqwest: HTTP client for API requests
- serde: JSON serialization/deserialization
- tokio: Async runtime
