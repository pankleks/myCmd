//! Small preview images for the image grid in the file viewer.
//!
//! Only formats that the `image` crate decodes are handled here. The frontend
//! shows HEIC, AVIF and SVG thumbnails from the regular preview instead.
use crate::error::{FsError, Result};
use crate::filesystem::absolute;
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader};
use std::{
    fs,
    io::{BufReader, Cursor},
    path::Path,
};

/// Photos are often larger than the 4 MiB limit of the full preview.
pub const MAX_THUMBNAIL_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
/// Decoded RGB8 at this size needs about 192 MiB.
const MAX_THUMBNAIL_SOURCE_PIXELS: u64 = 64_000_000;
const MIN_THUMBNAIL_SIDE: u32 = 16;
const MAX_THUMBNAIL_SIDE: u32 = 512;

fn supported_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|extension| {
            matches!(
                extension.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
            )
        })
}

fn decode_error(error: ImageError) -> FsError {
    match error {
        ImageError::Limits(_) => {
            FsError::new("image_too_large", "This image is too large for a thumbnail")
        }
        ImageError::Unsupported(_) => FsError::new(
            "unsupported_image",
            "This image format cannot be shown as a thumbnail",
        ),
        ImageError::IoError(error) => FsError::new("io_error", error.to_string()),
        _ => FsError::new("invalid_image", "This image could not be decoded"),
    }
}

/// Returns a data URL with a JPEG (or PNG for images with transparency) whose
/// longer side is at most `max_side` pixels. EXIF orientation is applied, as
/// the webview does for the full preview.
pub fn read_image_thumbnail(path: &Path, max_side: u32) -> Result<String> {
    if !supported_extension(path) {
        return Err(FsError::new(
            "unsupported_image",
            "This image format cannot be shown as a thumbnail",
        ));
    }
    let path = absolute(path)?;
    let metadata = fs::metadata(&path).map_err(|e| FsError::io(e, &path))?;
    if !metadata.is_file() {
        return Err(FsError::new("not_a_file", "Image source is not a file"));
    }
    if metadata.len() > MAX_THUMBNAIL_SOURCE_BYTES {
        return Err(FsError::new(
            "image_too_large",
            "Images larger than 64 MiB cannot be shown as thumbnails",
        ));
    }
    let side = max_side.clamp(MIN_THUMBNAIL_SIDE, MAX_THUMBNAIL_SIDE);

    let file = fs::File::open(&path).map_err(|e| FsError::io(e, &path))?;
    let reader = ImageReader::new(BufReader::new(file))
        .with_guessed_format()
        .map_err(|e| FsError::io(e, &path))?;
    if !matches!(
        reader.format(),
        Some(
            ImageFormat::Png
                | ImageFormat::Jpeg
                | ImageFormat::Gif
                | ImageFormat::WebP
                | ImageFormat::Bmp
        )
    ) {
        return Err(FsError::new(
            "invalid_image",
            "This file is not a supported image",
        ));
    }
    let mut decoder = reader.into_decoder().map_err(decode_error)?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 {
        return Err(FsError::new(
            "invalid_image",
            "Image dimensions must be nonzero",
        ));
    }
    if u64::from(width) * u64::from(height) > MAX_THUMBNAIL_SOURCE_PIXELS {
        return Err(FsError::new(
            "image_too_large",
            "This image is too large for a thumbnail",
        ));
    }
    // A broken EXIF block should not hide the picture.
    let orientation = decoder.orientation().ok();
    let mut image = DynamicImage::from_decoder(decoder).map_err(decode_error)?;
    if let Some(orientation) = orientation {
        image.apply_orientation(orientation);
    }
    if image.width() > side || image.height() > side {
        image = image.thumbnail(side, side);
    }

    let mut output = Cursor::new(Vec::new());
    let mime = if image.color().has_alpha() {
        DynamicImage::ImageRgba8(image.to_rgba8())
            .write_to(&mut output, ImageFormat::Png)
            .map_err(decode_error)?;
        "image/png"
    } else {
        DynamicImage::ImageRgb8(image.to_rgb8())
            .write_to(&mut output, ImageFormat::Jpeg)
            .map_err(decode_error)?;
        "image/jpeg"
    };
    Ok(format!(
        "data:{mime};base64,{}",
        STANDARD.encode(output.into_inner())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage, Rgba, RgbaImage};

    fn decode(data_url: &str) -> (String, DynamicImage) {
        let (header, data) = data_url.split_once(";base64,").unwrap();
        let bytes = STANDARD.decode(data).unwrap();
        (
            header.trim_start_matches("data:").to_string(),
            image::load_from_memory(&bytes).unwrap(),
        )
    }

    #[test]
    fn scales_opaque_images_down_to_a_jpeg_with_the_same_aspect_ratio() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("wide.PNG");
        RgbImage::from_pixel(800, 400, Rgb([200, 30, 30]))
            .save_with_format(&path, ImageFormat::Png)
            .unwrap();

        let (mime, image) = decode(&read_image_thumbnail(&path, 256).unwrap());
        assert_eq!(mime, "image/jpeg");
        assert_eq!((image.width(), image.height()), (256, 128));
    }

    #[test]
    fn keeps_transparency_as_png_and_does_not_upscale_small_images() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("icon.png");
        RgbaImage::from_pixel(40, 20, Rgba([0, 0, 0, 0]))
            .save_with_format(&path, ImageFormat::Png)
            .unwrap();

        let (mime, image) = decode(&read_image_thumbnail(&path, 256).unwrap());
        assert_eq!(mime, "image/png");
        assert_eq!((image.width(), image.height()), (40, 20));
        assert_eq!(image.to_rgba8().get_pixel(0, 0)[3], 0);
    }

    #[test]
    fn clamps_the_requested_size() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("photo.jpg");
        RgbImage::from_pixel(2000, 1000, Rgb([10, 120, 10]))
            .save_with_format(&path, ImageFormat::Jpeg)
            .unwrap();

        let (_, image) = decode(&read_image_thumbnail(&path, 10_000).unwrap());
        assert_eq!((image.width(), image.height()), (512, 256));
    }

    #[test]
    fn rejects_unsupported_corrupt_and_oversized_files() {
        let temp = tempfile::tempdir().unwrap();
        let svg = temp.path().join("vector.svg");
        fs::write(&svg, "<svg/>").unwrap();
        assert_eq!(
            read_image_thumbnail(&svg, 256).unwrap_err().code,
            "unsupported_image"
        );

        let corrupt = temp.path().join("broken.jpg");
        fs::write(&corrupt, b"not an image").unwrap();
        assert_eq!(
            read_image_thumbnail(&corrupt, 256).unwrap_err().code,
            "invalid_image"
        );

        let large = temp.path().join("large.png");
        fs::File::create(&large)
            .unwrap()
            .set_len(MAX_THUMBNAIL_SOURCE_BYTES + 1)
            .unwrap();
        assert_eq!(
            read_image_thumbnail(&large, 256).unwrap_err().code,
            "image_too_large"
        );

        assert_eq!(
            read_image_thumbnail(temp.path(), 256).unwrap_err().code,
            "unsupported_image"
        );
    }
}
