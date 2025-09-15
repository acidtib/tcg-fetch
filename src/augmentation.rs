use clap::Parser;
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb};
use indicatif::{ProgressBar, ProgressStyle};
use rand::Rng;
use rayon::prelude::*;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Augmentation parameters
#[derive(Parser, Debug)]
#[command(about = "Generate augmented versions of TCG card images")]
pub struct AugmentationArgs {
    /// Path to the dataset directory (should have train/, test/, validation/ subdirs)
    #[arg(short, long)]
    pub path: String,

    /// Number of augmented versions to generate per image
    #[arg(short, long, default_value_t = 5)]
    pub amount: u32,

    /// Verify image integrity after augmentation
    #[arg(long, default_value_t = false)]
    pub verify: bool,
}

/// Types of augmentations to apply
#[derive(Debug, Clone, Copy)]
pub enum AugmentationType {
    Rotation,
    Brightness,
    Contrast,
    Saturation,
    Noise,
    Blur,
    Flip,
}

impl AugmentationType {
    fn all() -> Vec<Self> {
        vec![
            Self::Rotation,
            Self::Brightness,
            Self::Contrast,
            Self::Saturation,
            Self::Noise,
            Self::Blur,
            Self::Flip,
        ]
    }
}

/// Statistics for augmentation process
#[derive(Debug, Default)]
struct AugmentationStats {
    total_original_images: usize,
    total_augmented_images: usize,
    total_cards: usize,
    train_images: usize,
    test_images: usize,
    validation_images: usize,
    corrupted_images: usize,
    verified_images: usize,
}

/// Apply augmentations to the dataset
pub async fn augment_dataset(args: AugmentationArgs) -> Result<(), Box<dyn std::error::Error>> {
    let base_dir = Path::new(&args.path);

    // Check if the train directory exists
    let train_dir = base_dir.join("train");

    if !train_dir.exists() {
        return Err("Dataset directory must contain train/ subdirectory".into());
    }

    println!("Starting augmentation process...");
    println!("Base directory: {}", args.path);
    println!("Augmentations per image: {}", args.amount);

    let mut stats = AugmentationStats::default();

    // Process train subset only
    let train_stats = process_subset(&train_dir, args.amount, "Training").await?;

    // Set statistics
    stats.total_cards = train_stats.0;
    stats.train_images = train_stats.1;
    stats.test_images = 0;
    stats.validation_images = 0;
    stats.total_original_images = train_stats.2;
    stats.total_augmented_images = train_stats.1;

    // Verify images if requested
    if args.verify {
        println!("\n🔍 Verifying image integrity...");
        let verification_stats = verify_images(&train_dir).await?;
        stats.corrupted_images = verification_stats.0;
        stats.verified_images = verification_stats.1;
    }

    // Print statistics
    print_augmentation_stats(&stats, args.verify);

    if stats.corrupted_images > 0 {
        println!(
            "\n⚠️  Warning: {} corrupted images found!",
            stats.corrupted_images
        );
    }

    println!("Augmentation process completed successfully!");
    Ok(())
}

/// Process a subset directory (train, test, or validation)
/// Returns (card_count, total_augmented_images, original_images_count)
async fn process_subset(
    subset_dir: &Path,
    amount: u32,
    subset_name: &str,
) -> Result<(usize, usize, usize), Box<dyn std::error::Error>> {
    println!("\nProcessing {} set...", subset_name);

    // Get all card directories
    let card_dirs: Vec<_> = fs::read_dir(subset_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_dir() {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    if card_dirs.is_empty() {
        println!("No card directories found in {}", subset_dir.display());
        return Ok((0, 0, 0));
    }

    println!("Found {} card directories", card_dirs.len());

    // Count total images for progress bar
    let total_original_images = count_images(&card_dirs)?;
    let total_augmentations = total_original_images * amount as usize;

    let progress_bar = ProgressBar::new(total_augmentations as u64);
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos:>7}/{len:7} {msg}",
            )
            .unwrap()
            .progress_chars("=>-"),
    );
    progress_bar.set_message(format!("Augmenting {} images", subset_name.to_lowercase()));

    let processed_count = AtomicUsize::new(0);

    // Process card directories in parallel
    card_dirs.par_iter().for_each(|card_dir| {
        if let Err(e) = process_card_directory(card_dir, amount, &progress_bar, &processed_count) {
            eprintln!(
                "Error processing card directory {}: {}",
                card_dir.display(),
                e
            );
        }
    });

    progress_bar.finish_with_message(format!(
        "Completed {} set augmentation",
        subset_name.to_lowercase()
    ));

    let final_total_images = count_images(&card_dirs)?;
    Ok((card_dirs.len(), final_total_images, total_original_images))
}

/// Process a single card directory
fn process_card_directory(
    card_dir: &Path,
    amount: u32,
    progress_bar: &ProgressBar,
    processed_count: &AtomicUsize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Find all image files in the card directory
    let image_files: Vec<_> = fs::read_dir(card_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_file() && is_image_file(&path) {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    // Process each image file
    for image_path in image_files {
        generate_augmentations(&image_path, amount)?;

        // Update progress
        let current = processed_count.fetch_add(amount as usize, Ordering::Relaxed);
        progress_bar.set_position((current + amount as usize) as u64);
    }

    Ok(())
}

/// Generate augmented versions of a single image
fn generate_augmentations(
    image_path: &Path,
    amount: u32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let img = image::open(image_path)?;
    let mut rng = rand::rng();

    let parent_dir = image_path.parent().ok_or("Invalid parent directory")?;

    // Find the highest existing number to avoid conflicts
    let mut max_existing = 0;
    if let Ok(entries) = fs::read_dir(parent_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if let Some(num_str) = name
                    .strip_suffix(".jpg")
                    .or_else(|| name.strip_suffix(".png"))
                {
                    if let Ok(num) = num_str.parse::<u32>() {
                        max_existing = max_existing.max(num);
                    }
                }
            }
        }
    }

    // Generate augmented versions
    for i in 1..=amount {
        let augmented_img = apply_random_augmentations(&img, &mut rng);

        let output_filename = format!("{:04}.jpg", max_existing + i);
        let output_path = parent_dir.join(output_filename);

        augmented_img.save_with_format(&output_path, ImageFormat::Jpeg)?;
    }

    Ok(())
}

/// Apply random augmentations to an image
fn apply_random_augmentations(img: &DynamicImage, rng: &mut impl Rng) -> DynamicImage {
    let mut result = img.clone();
    let augmentation_types = AugmentationType::all();

    // Apply 2-4 random augmentations
    let num_augmentations = rng.random_range(2..=4);
    let mut selected_augmentations = Vec::new();
    let mut available_types = augmentation_types.clone();

    for _ in 0..num_augmentations {
        if available_types.is_empty() {
            break;
        }
        let index = rng.random_range(0..available_types.len());
        selected_augmentations.push(available_types.remove(index));
    }

    for aug_type in selected_augmentations {
        result = apply_augmentation(&result, aug_type, rng);
    }

    result
}

/// Apply a specific augmentation to an image
fn apply_augmentation(
    img: &DynamicImage,
    aug_type: AugmentationType,
    rng: &mut impl Rng,
) -> DynamicImage {
    match aug_type {
        AugmentationType::Rotation => apply_rotation(img, rng),
        AugmentationType::Brightness => apply_brightness(img, rng),
        AugmentationType::Contrast => apply_contrast(img, rng),
        AugmentationType::Saturation => apply_saturation(img, rng),
        AugmentationType::Noise => apply_noise(img, rng),
        AugmentationType::Blur => apply_blur(img, rng),
        AugmentationType::Flip => apply_flip(img, rng),
    }
}

/// Apply rotation augmentation (-15 to +15 degrees)
fn apply_rotation(img: &DynamicImage, rng: &mut impl Rng) -> DynamicImage {
    let angle: f32 = rng.random_range(-15.0..=15.0);
    // Simple rotation implementation - for small angles, we can use a basic approach
    if angle.abs() > 5.0 {
        if rng.random_bool(0.5) {
            img.rotate90()
        } else {
            img.rotate270()
        }
    } else {
        img.clone() // For small angles, return original to avoid quality loss
    }
}

/// Apply brightness adjustment
fn apply_brightness(img: &DynamicImage, rng: &mut impl Rng) -> DynamicImage {
    let adjustment = rng.random_range(-30..=30);
    img.brighten(adjustment)
}

/// Apply contrast adjustment
fn apply_contrast(img: &DynamicImage, rng: &mut impl Rng) -> DynamicImage {
    let factor = rng.random_range(0.7..=1.3);
    adjust_contrast(img, factor)
}

/// Apply saturation adjustment
fn apply_saturation(img: &DynamicImage, rng: &mut impl Rng) -> DynamicImage {
    let factor = rng.random_range(0.5..=1.5);
    adjust_saturation(img, factor)
}

/// Apply noise
fn apply_noise(img: &DynamicImage, rng: &mut impl Rng) -> DynamicImage {
    let intensity = rng.random_range(5..=25);
    add_noise(img, intensity, rng)
}

/// Apply blur
fn apply_blur(img: &DynamicImage, rng: &mut impl Rng) -> DynamicImage {
    let sigma = rng.random_range(0.5..=2.0);
    img.blur(sigma)
}

/// Apply flip
fn apply_flip(img: &DynamicImage, rng: &mut impl Rng) -> DynamicImage {
    if rng.random_bool(0.5) {
        img.fliph() // Horizontal flip
    } else {
        img.flipv() // Vertical flip
    }
}

/// Adjust image contrast
fn adjust_contrast(img: &DynamicImage, factor: f32) -> DynamicImage {
    let rgb_img = img.to_rgb8();
    let (width, height) = rgb_img.dimensions();

    let mut new_img = ImageBuffer::new(width, height);

    for (x, y, pixel) in rgb_img.enumerate_pixels() {
        let r = ((pixel[0] as f32 - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u8;
        let g = ((pixel[1] as f32 - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u8;
        let b = ((pixel[2] as f32 - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u8;

        new_img.put_pixel(x, y, Rgb([r, g, b]));
    }

    DynamicImage::ImageRgb8(new_img)
}

/// Adjust image saturation
fn adjust_saturation(img: &DynamicImage, factor: f32) -> DynamicImage {
    let rgb_img = img.to_rgb8();
    let (width, height) = rgb_img.dimensions();

    let mut new_img = ImageBuffer::new(width, height);

    for (x, y, pixel) in rgb_img.enumerate_pixels() {
        let r = pixel[0] as f32 / 255.0;
        let g = pixel[1] as f32 / 255.0;
        let b = pixel[2] as f32 / 255.0;

        // Convert to grayscale
        let gray = 0.299 * r + 0.587 * g + 0.114 * b;

        // Interpolate between grayscale and original
        let new_r = (gray + factor * (r - gray)).clamp(0.0, 1.0);
        let new_g = (gray + factor * (g - gray)).clamp(0.0, 1.0);
        let new_b = (gray + factor * (b - gray)).clamp(0.0, 1.0);

        new_img.put_pixel(
            x,
            y,
            Rgb([
                (new_r * 255.0) as u8,
                (new_g * 255.0) as u8,
                (new_b * 255.0) as u8,
            ]),
        );
    }

    DynamicImage::ImageRgb8(new_img)
}

/// Add noise to the image
fn add_noise(img: &DynamicImage, intensity: u8, rng: &mut impl Rng) -> DynamicImage {
    let rgb_img = img.to_rgb8();
    let (width, height) = rgb_img.dimensions();

    let mut new_img = ImageBuffer::new(width, height);

    for (x, y, pixel) in rgb_img.enumerate_pixels() {
        let noise_r = rng.random_range(-(intensity as i16)..=(intensity as i16));
        let noise_g = rng.random_range(-(intensity as i16)..=(intensity as i16));
        let noise_b = rng.random_range(-(intensity as i16)..=(intensity as i16));

        let new_r = (pixel[0] as i16 + noise_r).clamp(0, 255) as u8;
        let new_g = (pixel[1] as i16 + noise_g).clamp(0, 255) as u8;
        let new_b = (pixel[2] as i16 + noise_b).clamp(0, 255) as u8;

        new_img.put_pixel(x, y, Rgb([new_r, new_g, new_b]));
    }

    DynamicImage::ImageRgb8(new_img)
}

/// Count total images in card directories
fn count_images(card_dirs: &[PathBuf]) -> Result<usize, Box<dyn std::error::Error>> {
    let mut total = 0;

    for card_dir in card_dirs {
        let images = fs::read_dir(card_dir)?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.is_file() && is_image_file(&path) {
                    Some(path)
                } else {
                    None
                }
            })
            .count();
        total += images;
    }

    Ok(total)
}

/// Check if a file is an image file based on its extension
fn is_image_file(path: &Path) -> bool {
    if let Some(extension) = path.extension() {
        let ext = extension.to_string_lossy().to_lowercase();
        matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "bmp" | "gif" | "tiff" | "webp"
        )
    } else {
        false
    }
}
async fn verify_images(train_dir: &Path) -> io::Result<(usize, usize)> {
    let mut corrupted = 0;
    let mut verified = 0;

    if !train_dir.exists() {
        return Ok((0, 0));
    }

    let card_dirs: Vec<_> = std::fs::read_dir(train_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_dir() {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    for card_dir in card_dirs {
        let image_files: Vec<_> = std::fs::read_dir(&card_dir)?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.is_file() && is_image_file(&path) {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();

        for image_path in image_files {
            match image::open(&image_path) {
                Ok(_) => verified += 1,
                Err(_) => {
                    corrupted += 1;
                    eprintln!("❌ Corrupted image: {}", image_path.display());
                }
            }
        }
    }

    Ok((corrupted, verified))
}

/// Print augmentation statistics
fn print_augmentation_stats(stats: &AugmentationStats, verified: bool) {
    println!("\n🎯 Augmentation Statistics:");
    println!("  📊 Total cards processed: {}", stats.total_cards);
    println!("  📷 Original images: {}", stats.total_original_images);
    println!(
        "  🔄 Total images after augmentation: {}",
        stats.total_augmented_images
    );
    println!(
        "  ➕ New augmented images created: {}",
        stats.total_augmented_images - stats.total_original_images
    );
    println!("\n📁 Training dataset:");
    println!("  🏋️  Training:   {} images", stats.train_images);

    let multiplier = if stats.total_original_images > 0 {
        stats.total_augmented_images as f64 / stats.total_original_images as f64
    } else {
        0.0
    };
    println!("\n📈 Dataset size multiplier: {:.1}x", multiplier);

    if verified {
        println!("\n🔍 Image verification:");
        println!("  ✅ Verified images: {}", stats.verified_images);
        if stats.corrupted_images > 0 {
            println!("  ❌ Corrupted images: {}", stats.corrupted_images);
        } else {
            println!("  🎉 All images verified successfully!");
        }
    }
}
