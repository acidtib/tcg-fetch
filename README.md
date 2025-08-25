# Ojo Fetch Magic

A Rust CLI tool to fetch Magic: The Gathering card data from the Scryfall API. This tool helps organize card data into train, test, and validation sets for machine learning applications.

## Features

- Fetches card data from Scryfall API
- Downloads different types of card data (unique artwork, oracle cards, default cards, or all cards)
- Direct image resizing to specified dimensions (default: 384×512 pixels)
- Organizes data into train/test/validation sets with automatic splitting
- Command-line interface with customizable output path
- Checks for existing files to avoid unnecessary downloads
- Progress indicators for downloads
- Multi-threaded image downloading with configurable thread count

## Installation

Make sure you have Rust and Cargo installed on your system. Then clone this repository:

```bash
git clone https://github.com/acidtib/ojo-fetch-magic.git
cd ojo-fetch-magic
cargo build --release
```

## Usage

### Basic Usage

Run the program with default settings (fetches default cards):

```bash
cargo run
```

### Data Type Selection

Specify the type of data to fetch using the `--data` or `-d` flag:

```bash
cargo run -- --data unique    # Download unique artwork data
cargo run -- --data oracle    # Download oracle cards data
cargo run -- --data default   # Download default cards data
cargo run -- --data all       # Download all cards data
```

### Image Processing Options

Configure target image dimensions (images will be resized to fit exactly):

```bash
cargo run -- --width 512 --height 512    # Resize all images to 512×512 pixels
```

### Performance Tuning

Control download performance and dataset size:

```bash
cargo run -- --amount 100 --threads 6    # Download 100 cards using 6 threads
```

### Custom Output Directory

Specify a custom output directory:

```bash
cargo run -- --path custom-data-dir
```

### Complete Example

Combine multiple options for full control:

```bash
cargo run -- --data oracle --path custom-data-dir --amount 50 --threads 4 --width 512 --height 512
```

## Output Structure

The tool creates a directory with the following structure:

```
<output-dir>/
├── data/
│   ├── train/
│   │   └── <card-id>.jpg    # Original image
│   ├── test/
│   │   └── <card-id>.jpg    # Test image
│   ├── validation/
│   │   └── <card-id>.jpg    # Validation image
└── <data-type>.json
```

## Command Line Options

```
Options:
  -p, --path <PATH>              Path where to save the data [default: magic-data]
  -d, --data <DATA>              Type of card data to fetch [default: default] [possible values: unique, oracle, default, all]
  -a, --amount <AMOUNT>          Amount of cards to fetch [default: all]
  -t, --threads <THREADS>        Number of threads to use for downloading images [default: CPU cores]
      --width <WIDTH>            Target width for resized images [default: 384]
      --height <HEIGHT>          Target height for resized images [default: 512]
  -h, --help                     Print help
  -V, --version                  Print version
```

## Dependencies

### Rust Dependencies
- clap: Command-line argument parsing
- reqwest: HTTP client for API requests
- serde: JSON serialization/deserialization
- tokio: Async runtime
- image: Image processing
- indicatif: Progress bars
- futures: Async utilities
- rand: Random number generation for dataset shuffling

## Project Structure

```
ojo-fetch-magic/
├── src/
│   ├── main.rs           # Main application logic
│   └── utils.rs          # Utility functions for API and file operations
├── Cargo.toml           # Rust dependencies
└── README.md           # This file
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is open source. Please check the repository for license details.