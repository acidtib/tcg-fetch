use image::{Rgb, RgbImage};
use imageproc::geometric_transformations::{rotate_about_center, Interpolation};
use rand::Rng;
use std::fs;
use std::path::Path;

/// Generates augmented versions of an input image
///
/// # Arguments
/// * `img_path` - Path to the source image
/// * `save_dir` - Directory where augmented images will be saved
/// * `total_number` - Number of augmented images to generate (default: 5)
///
/// # Returns
/// * `bool` - true if new images were generated, false if skipped due to existing images
pub fn generate_augmented_images<P: AsRef<Path>>(
    img_path: P,
    save_dir: P,
    total_number: Option<u32>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let total = total_number.unwrap_or(5);

    // Check if directory already has required augmented images
    let existing_images = fs::read_dir(&save_dir)?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map_or(false, |ext| ext == "jpg")
        })
        .count();

    if existing_images >= total as usize {
        return Ok(false);
    }

    // Load the original image
    let img = image::open(&img_path)?;
    let img_rgb = img.to_rgb8();

    let mut rng = rand::rng();

    for i in 0..total {
        let mut augmented = img_rgb.clone();

        // First augmented image (0001.jpg) is upside-down
        if i == 0 {
            // Rotate 180 degrees to flip upside-down
            augmented = rotate_about_center(
                &augmented,
                std::f32::consts::PI,
                Interpolation::Bilinear,
                Rgb([0, 0, 0]),
            );
        } else {
            // Apply random rotation (-10 to 10 degrees) for other images
            let rotation: f32 = rng.random_range(-10.0..10.0);
            augmented = rotate_about_center(
                &augmented,
                rotation.to_radians(),
                Interpolation::Bilinear,
                Rgb([0, 0, 0]),
            );

            // Apply random zoom (0.95 to 1.05)
            let zoom: f32 = rng.random_range(0.95..1.05);
            let (width, height) = augmented.dimensions();
            let new_width = (width as f32 * zoom) as u32;
            let new_height = (height as f32 * zoom) as u32;
            let resized = image::imageops::resize(
                &augmented,
                new_width,
                new_height,
                image::imageops::FilterType::Lanczos3,
            );

            // Apply small random shifts
            let shift_x: f32 = rng.random_range(-0.05..0.05) * width as f32;
            let shift_y: f32 = rng.random_range(-0.05..0.05) * height as f32;
            let mut shifted = RgbImage::new(width, height);
            image::imageops::overlay(&mut shifted, &resized, shift_x as i64, shift_y as i64);
            augmented = shifted;
        }

        // Save the augmented image
        let output_path = save_dir.as_ref().join(format!("{:04}.jpg", i + 1));

        augmented.save(output_path)?;
    }

    Ok(true)
}
