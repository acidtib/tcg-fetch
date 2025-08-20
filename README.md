# Ojo Fetch Magic

A Rust CLI tool to fetch Magic: The Gathering card data from the Scryfall API. This tool helps organize card data into train, test, and validation sets, with built-in image augmentation capabilities and Hugging Face dataset preparation.

## Features

- Fetches card data from Scryfall API
- Downloads different types of card data (unique artwork, oracle cards, default cards, or all cards)
- Customizable image processing with configurable width and height
- Optional automatic image augmentation for training data:
  - First augmented image is upside-down (180° rotation)
  - Random rotation (-10° to 10°) for other augmented versions
  - Slight zoom variations (95% to 105%)
  - Small position shifts (-5% to 5%)
  - Configurable number of augmented versions per card (default: 5)
- Organizes data into train/test/validation sets with automatic splitting
- Command-line interface with customizable output path
- Checks for existing files to avoid unnecessary downloads
- Progress indicators for downloads and augmentation
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

Configure image dimensions and processing:

```bash
cargo run -- --width 512 --height 512    # Set custom image dimensions
```

### Augmentation Controls

Enable image augmentation for training data:

```bash
cargo run -- --augmented                           # Enable augmentation with default settings
cargo run -- --augmented --augment-count 10        # Generate 10 augmented images per card
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
cargo run -- --data oracle --path custom-data-dir --amount 50 --threads 4 --augmented --augment-count 8 --width 512 --height 512
```

## Output Structure

The tool creates a directory with the following structure:

```
<output-dir>/
├── data/
│   ├── train/
│   │   └── <card-id>/
│   │       ├── 0000.jpg    # Original image
│   │       ├── 0001.jpg    # Upside-down version
│   │       ├── 0002.jpg    # Augmented version 2
│   │       ├── 0003.jpg    # Augmented version 3
│   │       ├── 0004.jpg    # Augmented version 4
│   │       └── 0005.jpg    # Augmented version 5
│   ├── test/
│   └── validation/
└── <data-type>.json
```

## Command Line Options

```
Options:
  -p, --path <PATH>              Path where to save the data [default: magic-data]
  -d, --data <DATA>              Type of card data to fetch [default: default] [possible values: unique, oracle, default, all]
  -a, --amount <AMOUNT>          Amount of cards to fetch [default: all]
  -t, --threads <THREADS>        Number of threads to use for downloading images [default: CPU cores]
      --augmented                Generate augmented images for training data
      --augment-count <COUNT>    Number of augmented images to generate per original image [default: 5]
      --width <WIDTH>            Width for processed images [default: 298]
      --height <HEIGHT>          Height for processed images [default: 298]
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
- imageproc: Image transformations
- rand: Random number generation for augmentation
- indicatif: Progress bars
- futures: Async utilities

## Project Structure

```
ojo-fetch-magic/
├── src/
│   ├── main.rs           # Main application logic
│   ├── utils.rs          # Utility functions for API and file operations
│   └── augment.rs        # Image augmentation functionality
├── Cargo.toml           # Rust dependencies
└── README.md           # This file
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is open source. Please check the repository for license details.
