# Ojo Fetch Magic

A Rust CLI tool to fetch Magic: The Gathering card data from the Scryfall API. This tool helps organize card data into train, test, and validation sets.

## Features

- Fetches card data from Scryfall API
- Organizes data into train/test/validation sets
- Command-line interface with customizable output path

## Installation

Make sure you have Rust and Cargo installed on your system. Then clone this repository:

```bash
git clone https://github.com/acidtib/ojo-fetch-magic.git
cd ojo-fetch-magic
cargo build --release
```

## Usage

Run the program with default settings:

```bash
cargo run
```

This will create a `magic-data` directory with the following structure:
```
magic-data/
├── data/
│   ├── train/
│   ├── test/
│   └── valid/
└── magic-cards.json
```

Specify a custom output directory:

```bash
cargo run -- --path custom-data-dir
```

For help and available options:

```bash
cargo run -- --help
```

## Dependencies

- `clap` - Command line argument parsing
