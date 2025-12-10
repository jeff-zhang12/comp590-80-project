use std::process::Command;
use std::io;
use std::fs;
use ffmpeg_next as ffmpeg;
use ffmpeg::frame;
use image::{RgbaImage, Rgba};

use crate::video::FrameIter;

pub fn compress_dynamic_roi(input: &str, output: &str, bboxes: &[(i32, i32, i32, i32)]) -> io::Result<()> {
    println!("Starting hybrid dynamic ROI compression...");
    println!("Input: {}", input);
    println!("Output: {}", output);
    println!("Bounding boxes: {}", bboxes.len());

    // Create temp directory for frames
    let temp_dir = "temp_frames";
    fs::create_dir_all(temp_dir)?;

    // Step 1: Extract frames and create ROI overlays using ffmpeg-next
    println!("\nStep 1: Extracting frames and ROIs with ffmpeg-next...");
    extract_roi_frames(input, temp_dir, bboxes)?;

    // Step 2: Create compressed background video using Command
    println!("\nStep 2: Creating compressed background with Command...");
    let bg_temp = format!("{}/bg_temp.mp4", temp_dir);
    let status_bg = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(input)
        .arg("-c:v")
        .arg("libx264")
        .arg("-crf")
        .arg("40") // Heavy compression for background
        .arg("-preset")
        .arg("medium")
        .arg("-an") // No audio in temp file
        .arg(&bg_temp)
        .status()?;

    if !status_bg.success() {
        return Err(io::Error::other("Failed to create compressed background"));
    }

    // Step 3: Overlay high-quality ROIs onto compressed background
    println!("\nStep 3: Compositing ROIs onto background...");
    let roi_pattern = format!("{}/roi_%05d.png", temp_dir);
    
    let status_final = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(&bg_temp)
        .arg("-i")
        .arg(&roi_pattern)
        .arg("-filter_complex")
        .arg("[0:v][1:v]overlay=0:0:format=auto")
        .arg("-c:v")
        .arg("libx264")
        .arg("-crf")
        .arg("23") // Good quality for final output
        .arg("-preset")
        .arg("medium")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-c:a")
        .arg("copy")
        .arg("-shortest")
        .arg(output)
        .status()?;

    // Cleanup temp directory
    println!("\nStep 4: Cleaning up temporary files...");
    let _ = fs::remove_dir_all(temp_dir);

    if status_final.success() {
        println!("✓ Compression completed successfully!");
        Ok(())
    } else {
        Err(io::Error::other("Failed to composite final video"))
    }
}

fn extract_roi_frames(
    input: &str,
    temp_dir: &str,
    bboxes: &[(i32, i32, i32, i32)]
) -> io::Result<()> {
    // Initialize FFmpeg
    ffmpeg::init().map_err(|e| io::Error::other(format!("FFmpeg init failed: {}", e)))?;

    let frames = FrameIter::new(input)
        .map_err(|e| io::Error::other(format!("Failed to open video: {}", e)))?;

    // Get video dimensions from first frame
    let mut frame_count = 0;
    
    for (frame, bbox) in frames.zip(bboxes.iter()) {
        if frame_count % 30 == 0 {
            println!("  Processing frame {}/{}", frame_count, bboxes.len());
        }

        // Create transparent PNG with only ROI visible
        let roi_path = format!("{}/roi_{:05}.png", temp_dir, frame_count);
        save_roi_as_transparent_png(&frame, bbox, &roi_path)?;

        frame_count += 1;
    }

    println!("  ✓ Extracted {} ROI frames", frame_count);
    Ok(())
}

fn save_roi_as_transparent_png(
    frame: &frame::Video,
    bbox: &(i32, i32, i32, i32),
    output_path: &str
) -> io::Result<()> {
    let (x1, y1, x2, y2) = bbox;
    let width = frame.width();
    let height = frame.height();

    // Create fully transparent RGBA image
    let mut img = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));

    // Get frame data (assume RGB or similar format)
    let data = frame.data(0);
    let stride = frame.stride(0);

    // Clamp bbox to frame dimensions
    let x1 = (*x1).max(0).min(width as i32 - 1) as u32;
    let y1 = (*y1).max(0).min(height as i32 - 1) as u32;
    let x2 = (*x2).max(0).min(width as i32) as u32;
    let y2 = (*y2).max(0).min(height as i32) as u32;

    // Copy only the ROI region with full opacity
    for y in y1..y2 {
        for x in x1..x2 {
            let idx = (y as usize * stride + x as usize * 3) as usize;
            if idx + 2 < data.len() {
                let r = data[idx];
                let g = data[idx + 1];
                let b = data[idx + 2];
                img.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }
    }

    img.save(output_path)
        .map_err(|e| io::Error::other(format!("Failed to save PNG: {}", e)))
}
