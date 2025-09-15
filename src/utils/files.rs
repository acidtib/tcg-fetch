use crate::tcg::TcgType;
use rayon::prelude::*;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Ensure required directories exist for TCG data storage
pub fn ensure_directories(base_path: &str) -> io::Result<()> {
    let base_path = Path::new(base_path);

    let dirs_to_create = vec![
        base_path.to_path_buf(),
        base_path.join("data"),
        base_path.join("data/train"),
    ];

    // Check which directories don't exist
    let missing_dirs: Vec<PathBuf> = dirs_to_create
        .into_par_iter()
        .filter(|dir| !dir.exists())
        .collect();

    // Create missing directories in parallel
    missing_dirs
        .par_iter()
        .try_for_each(|dir| -> io::Result<()> {
            fs::create_dir_all(dir)?;
            println!("Created directory: {}", dir.display());
            Ok(())
        })?;

    println!("All required directories are ready!");
    Ok(())
}

/// Check for existing JSON files for a specific TCG type
pub fn check_json_files(directory: &str, tcg_type: &TcgType) -> Vec<String> {
    let base_path = Path::new(directory);
    let mut existing_files = Vec::new();

    // Check for the specific TCG type file
    let file_type = match tcg_type {
        TcgType::Mtg => "mtg_cards",
        TcgType::Ga => "ga_cards",
    };

    let file_path = base_path.join(format!("{}.json", file_type));
    if file_path.exists() {
        existing_files.push(file_path.to_string_lossy().into_owned());
    }

    existing_files
}

/// Count and display the number of directories in the train folder
pub fn count_train_directories(base_path: &str) -> io::Result<()> {
    let train_path = Path::new(base_path).join("data/train");

    if !train_path.exists() {
        println!("Train directory does not exist yet.");
        return Ok(());
    }

    let count = fs::read_dir(&train_path)?
        .filter_map(|entry| {
            entry.ok().and_then(|e| {
                if e.file_type().ok()?.is_dir() {
                    Some(())
                } else {
                    None
                }
            })
        })
        .count();

    println!("Total card directories in train folder: {}", count);
    Ok(())
}

// TODO: Add tests with proper test dependencies
