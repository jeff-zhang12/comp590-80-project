use std::process::Command;
use std::io;
use std::fs;
use std::thread;
use ffmpeg_next as ffmpeg;
use ffmpeg::frame;
use image::{RgbaImage, Rgba};
use crossbeam_channel::bounded;

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
    println!("\nStep 1: Extracting frames and ROIs with ffmpeg-next...");
    let fps = extract_roi_frames(input, temp_dir, bboxes)?;

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
        .arg("51") // Heavy compression for background (Max CRF)
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

        .arg("-framerate")
        .arg(format!("{}", fps))
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

) -> io::Result<f64> {
    // FrameIter::new already calls ffmpeg::init()
    let frames = FrameIter::new(input)
        .map_err(|e| io::Error::other(format!("Failed to open video: {}", e)))?;
    
    let fps = frames.fps();

    // Create a bounded channel for parallel saving
    // Capacity 32 ensures we don't consume too much RAM if saving is slow,
    // but gives enough buffer for the decoder to run ahead.
    let (sender, receiver) = bounded::<(RgbaImage, String)>(32);

    // Spawn a thread to handle file saving
    let save_thread = thread::spawn(move || {
        while let Ok((img, path)) = receiver.recv() {
            if let Err(e) = img.save(&path) {
                eprintln!("Failed to save frame to {}: {}", path, e);
            }
        }
    });

    let mut frame_count = 0;
    
    // We iterate sequentially, but save in parallel
    for (frame, bbox) in frames.zip(bboxes.iter()) {
        if frame_count % 30 == 0 {
            println!("  Processing frame {}/{}", frame_count, bboxes.len());
        }

        // Extract ROI to memory (this is fast)
        let roi_img = extract_roi_to_image(&frame, bbox)?;
        let roi_path = format!("{}/roi_{:05}.png", temp_dir, frame_count);

        // Send to saver thread (blocks if channel full, providing backpressure)
        if let Err(_) = sender.send((roi_img, roi_path)) {
            break; // Receiver disconnected, stop processing
        }

        frame_count += 1;
    }

    // Drop sender to signal end of stream to receiver
    drop(sender);

    // Wait for all files to be saved
    if let Err(_) = save_thread.join() {
        return Err(io::Error::other("Saver thread panicked"));
    }

    println!("  ✓ Extracted {} ROI frames", frame_count);
    println!("  ✓ Extracted {} ROI frames", frame_count);
    Ok(fps)
}

fn extract_roi_to_image(
    frame: &frame::Video,
    bbox: &(i32, i32, i32, i32)
) -> io::Result<RgbaImage> {
    let (x1, y1, x2, y2) = bbox;
    let width = frame.width();
    let height = frame.height();

    // Create fully transparent RGBA image (initialized to 0)
    // We must return a full-size image so that overlay=0:0 works correctly in ffmpeg
    let mut img = RgbaImage::new(width, height);

    // Get frame data (assume RGB or similar format)
    let data = frame.data(0);
    let stride = frame.stride(0);

    // Clamp bbox to frame dimensions
    let x1 = (*x1).max(0).min(width as i32 - 1) as usize;
    let y1 = (*y1).max(0).min(height as i32 - 1) as usize;
    let x2 = (*x2).max(0).min(width as i32) as usize;
    let y2 = (*y2).max(0).min(height as i32) as usize;

    let roi_width = x2 - x1;

    // Use raw buffer for simpler indexing and to avoid Index trait issues
    let mut buffer = img.into_raw();
    let img_width = width as usize;
    
    for y in y1..y2 {
        let src_idx = y * stride + x1 * 3;
        
        // Calculate destination index in the flat RGBA buffer
        // Image is packed: (y * width + x) * 4
        let dst_idx = (y * img_width + x1) * 4;

        if src_idx + roi_width * 3 <= data.len() {
             let src_row = &data[src_idx..src_idx + roi_width * 3];
             
             // Get mutable slice of the destination row from the raw buffer
             if dst_idx + roi_width * 4 <= buffer.len() {
                 let dst_slice = &mut buffer[dst_idx..dst_idx + roi_width * 4];
                 
                 // Copy loop: 3 bytes src -> 4 bytes dst (add alpha)
                 for (chunk_src, chunk_dst) in src_row.chunks(3).zip(dst_slice.chunks_mut(4)) {
                     chunk_dst[0] = chunk_src[0];
                     chunk_dst[1] = chunk_src[1];
                     chunk_dst[2] = chunk_src[2];
                     chunk_dst[3] = 255; // Full opacity
                 }
             }
        }
    }

    RgbaImage::from_raw(width, height, buffer)
        .ok_or_else(|| io::Error::other("Failed to recreate image from buffer"))
}

pub fn compress_standard(input: &str, output: &str, crf: u8) -> io::Result<()> {
    println!("Starting standard compression...");
    println!("Input: {}", input);
    println!("Output: {}", output);
    println!("CRF: {}", crf);

    let status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(input)
        .arg("-c:v")
        .arg("libx264")
        .arg("-crf")
        .arg(format!("{}", crf))
        .arg("-preset")
        .arg("medium")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-c:a")
        .arg("copy")
        .arg(output)
        .status()?;

    if status.success() {
        println!("✓ Standard compression completed successfully!");
        Ok(())
    } else {
        Err(io::Error::other("Failed to perform standard compression"))
    }
}
