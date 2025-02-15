# Ojo Fetch Magic

A Rust CLI tool to fetch Magic: The Gathering card data from the Scryfall API. This tool helps organize card data into train, test, and validation sets, with built-in image augmentation capabilities.

## Features

- Fetches card data from Scryfall API
- Downloads different types of card data (unique artwork, oracle cards, default cards, or all cards)
- Automatic image augmentation for each downloaded card:
  - Random rotation (-10° to 10°)
  - Slight zoom variations (95% to 105%)
  - Small position shifts
  - Generates 5 augmented versions per card
- Organizes data into train/test/validation sets
- Command-line interface with customizable output path
- Checks for existing files to avoid unnecessary downloads
- Progress indicator for downloads and augmentation
- Multi-threaded image downloading

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
cargo run -- --data all       # Download all cards data
```

Limit the number of cards to download and set thread count:

```bash
cargo run -- --amount 100 --threads 6    # Download 100 cards using 6 threads
```

Specify a custom output directory:

```bash
cargo run -- --path custom-data-dir
```

Combine multiple options:

```bash
cargo run -- --data oracle --path custom-data-dir --amount 50 --threads 4
```

This will create a directory with the following structure:
```
<output-dir>/
├── data/
│   ├── train/
│   │   └── <card-id>/
│   │       ├── 0000.jpg    # Original image
│   │       ├── 0001.jpg    # Augmented version 1
│   │       ├── 0002.jpg    # Augmented version 2
│   │       ├── 0003.jpg    # Augmented version 3
│   │       ├── 0004.jpg    # Augmented version 4
│   │       └── 0005.jpg    # Augmented version 5
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
- image: Image processing
- imageproc: Image transformations
- rand: Random number generation for augmentation
- indicatif: Progress bars
