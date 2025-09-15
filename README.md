# TCG Fetch

A high-performance Rust CLI tool to fetch trading card game data from various APIs. This tool helps organize card data into a training dataset for machine learning applications.

## Supported TCGs

- **Magic: The Gathering (MTG)** - Fetches data from the Scryfall API
- **Grand Archive (GA)** - Fetches data from the Grand Archive API

## Features

- **Fast parallel processing** - Uses all CPU cores for optimal performance
- **Smart batch checking** - Avoids unnecessary downloads by checking existing cards efficiently
- Fetches card data from TCG-specific APIs
- Downloads comprehensive card data including all available card information
- Direct image resizing to specified dimensions (default: 500×700 pixels)
- Organizes data into a training dataset structure
- Command-line interface with customizable output path
- Progress indicators for downloads
- Multi-threaded image downloading with configurable thread count
- **Image augmentation** - Generate multiple augmented versions of card images for improved ML training

## Installation

Make sure you have Rust and Cargo installed on your system. Then clone this repository:

```bash
git clone https://github.com/acidtib/tcg-fetch.git
cd tcg-fetch
cargo build --release
```

## Usage

The tool provides two main commands:
1. `fetch` - Download card data and images from TCG APIs
2. `augment` - Generate augmented versions of existing card images

### Fetching Card Data

Run the program with the fetch command and required TCG argument:

```bash
cargo run -- fetch mtg    # For Magic: The Gathering
cargo run -- fetch ga     # For Grand Archive
```

### Image Processing Options

Configure target image dimensions (images will be resized to fit exactly):

```bash
cargo run -- fetch mtg --width 512 --height 512    # Resize all MTG images to 512×512 pixels
cargo run -- fetch ga --width 512 --height 512     # Resize all GA images to 512×512 pixels
```

### Performance Tuning

Control download performance and dataset size:

```bash
cargo run -- fetch mtg --amount 100 --threads 6    # Download 100 MTG cards using 6 threads
cargo run -- fetch ga --amount 50 --threads 4      # Download 50 GA cards using 4 threads
```

### Custom Output Directory

Specify a custom output directory:

```bash
cargo run -- fetch mtg --path custom-data-dir      # For MTG cards
cargo run -- fetch ga --path ga-cards-dir          # For GA cards
```

### Complete Example

Combine multiple options for full control:

```bash
cargo run -- fetch mtg --path mtg-data --amount 50 --threads 4 --width 512 --height 512
cargo run -- fetch ga --path ga-data --amount 25 --threads 2 --width 400 --height 600
```

### Image Augmentation

After fetching card data, you can generate augmented versions to improve machine learning training:

```bash
# Generate 5 augmented versions per image (default)
cargo run -- augment --path tcg-data/data

# Generate 10 augmented versions per image
cargo run -- augment --path tcg-data/data --amount 10

# Augment a specific dataset with image verification
cargo run -- augment --path ./my-cards/data --amount 8 --verify
```

The augmentation process applies random combinations of the following transformations:
- **Rotation** - Small rotations (-15° to +15°)
- **Brightness** - Brightness adjustments (-30 to +30)
- **Contrast** - Contrast modifications (0.7x to 1.3x)
- **Saturation** - Saturation changes (0.5x to 1.5x)
- **Noise** - Random noise addition (5-25 intensity)
- **Blur** - Gaussian blur (0.5-2.0 sigma)
- **Flip** - Horizontal or vertical flipping

Each augmented image receives 2-4 random transformations to create realistic variations while preserving card readability.

The augmentation process includes:
- **Smart file naming** - Automatically finds the highest existing image number to avoid conflicts
- **Parallel processing** - Uses all CPU cores for optimal performance
- **Progress tracking** - Shows real-time progress with detailed statistics
- **Image verification** - Optional integrity checking to ensure all generated images are valid
- **Comprehensive statistics** - Detailed reporting of augmentation results

## Output Structure

The fetch command creates a directory with the following structure:

```
<output-dir>/
├── data/
│   └── train/
│       └── <card-id>/
│           └── 0000.jpg     # Original downloaded image
├── mtg_cards.json           # For Magic: The Gathering
└── ga_cards.json            # For Grand Archive
```

Each card gets its own subdirectory named after the card ID. The primary image is saved as `0000.jpg`.

After running the augment command, additional augmented images are added:

```
<output-dir>/data/
└── train/
    └── <card-id>/
        ├── 0000.jpg     # Original image
        ├── 0001.jpg     # Augmented version 1
        ├── 0002.jpg     # Augmented version 2
        └── ...          # Additional augmented versions
```

### Advanced Features

The augmentation process provides several advanced features:

**Statistical Reporting**: After completion, the tool provides comprehensive statistics including:
- Total cards and images processed
- Dataset size multipliers
- Per-dataset image counts
- Verification results (if enabled)

**Image Verification**: Use the `--verify` flag to check image integrity after augmentation:
```bash
cargo run -- augment --directory tcg-data/data --amount 5 --verify
```

This will verify that all generated images can be opened successfully and report any corrupted files.

## Command Line Options

```
Usage: tcg-fetch <COMMAND>

Commands:
  fetch    Fetch trading card game data from various APIs
  augment  Generate augmented versions of TCG card images
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### Fetch Command Options

```
Usage: tcg-fetch fetch <TCG> [OPTIONS]

Arguments:
  <TCG>                          Trading card game type to fetch data for [possible values: mtg, ga]

Options:
  -p, --path <PATH>              Path where to save the data [default: tcg-data]
  -a, --amount <AMOUNT>          Amount of cards to fetch [default: all]
  -t, --threads <THREADS>        Number of threads to use for downloading images [default: CPU cores]
      --width <WIDTH>            Target width for resized images [default: 500]
      --height <HEIGHT>          Target height for resized images [default: 700]
  -h, --help                     Print help
```

### Augment Command Options

```
Usage: tcg-fetch augment [OPTIONS] --path <PATH>

Options:
  -p, --path <PATH>              Path to the dataset directory (should have train/ subdir)
  -a, --amount <AMOUNT>          Number of augmented versions to generate per image [default: 5]
      --verify                   Verify image integrity after augmentation
  -h, --help                     Print help
```

## Quick Start Examples

### Example 1: Basic Workflow
```bash
# Fetch 50 MTG cards
cargo run -- fetch mtg --path my-cards --amount 50

# Generate 3 augmented versions per image
cargo run -- augment --path my-cards/data --amount 3

# Result: ~200 total training images (original + augmented)
```

### Example 2: High-Quality Dataset
```bash
# Fetch 100 cards with high resolution
cargo run -- fetch mtg --path hq-dataset --amount 100 --width 600 --height 800

# Generate 5 augmented versions with verification
cargo run -- augment --path hq-dataset/data --amount 5 --verify

# Result: 600 high-quality training images
```

### Example 3: Production Pipeline
```bash
# Step 1: Fetch large dataset
cargo run -- fetch mtg --path production --amount 1000 --threads 8

# Step 2: Apply augmentation for ML training
cargo run -- augment --path production/data --amount 10 --verify

# Result: 11,000 verified training images (1 original + 10 augmented)
```

## Troubleshooting

### Common Issues

**Error: "Dataset directory must contain train/ subdirectory"**
- Solution: First run the `fetch` command to create the proper directory structure

**Slow augmentation performance:**
- The tool automatically uses all CPU cores
- Ensure sufficient disk space (augmented datasets can be 5-10x larger)
- Use `--verify` sparingly for large datasets as it adds processing time

**Image quality concerns:**
- Augmentation applies realistic transformations that preserve card readability
- Each image receives 2-4 random transformations to maintain diversity
- Original images are always preserved (0000.jpg files remain untouched)

### Advanced Usage

**Custom augmentation strategies:**
The current implementation applies random combinations of transformations. For specific augmentation strategies, you can run multiple augmentation passes with different amounts:

```bash
# Light augmentation
cargo run -- augment --path data --amount 2

# Add more variety
cargo run -- augment --path data --amount 3
```

**Batch processing multiple TCGs:**
```bash
# Fetch different TCG types
cargo run -- fetch mtg --path mtg-data --amount 500
cargo run -- fetch ga --path ga-data --amount 200

# Augment each dataset
cargo run -- augment --path mtg-data/data --amount 5
cargo run -- augment --path ga-data/data --amount 5
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. When adding support for new TCGs, please ensure:

- The new TCG follows the existing pattern in the codebase
- API rate limits are respected
- Error handling is comprehensive
- Documentation is updated accordingly

## License

This project is open source. Please check the repository for license details.
