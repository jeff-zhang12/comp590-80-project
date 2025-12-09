mod video;
mod processing;

use std::fs::File;
use std::io::{BufRead, BufReader};
use anyhow::{Context, Result};
use video::FrameIter;
use processing::compress_dynamic_roi;

fn load_bounding_boxes(path: &str) -> Result<Vec<(i32, i32, i32, i32)>> {
    let file = File::open(path).context("could not open bounding_boxes.txt")?;
    let reader = BufReader::new(file);
    let mut bounding_boxes = Vec::new();

    for line in reader.lines().map_while(Result::ok) {
        let parts: Vec<&str> = line.trim().split(',').collect();
        if parts.len() == 4 {
            let parsed = (
                parts[0].parse::<i32>(),
                parts[1].parse::<i32>(),
                parts[2].parse::<i32>(),
                parts[3].parse::<i32>(),
            );
            if let (Ok(x1), Ok(y1), Ok(x2), Ok(y2)) = parsed {
                bounding_boxes.push((x1, y1, x2, y2));
            }
        }
    }
    Ok(bounding_boxes)
}

fn main() -> Result<()> {
    let bounding_boxes = load_bounding_boxes("bounding_boxes.txt")?;

    // Run differential compression
    if let Err(e) = compress_dynamic_roi("videos/input.mp4", "videos/output_dynamic.mp4", &bounding_boxes) {
        eprintln!("Error during compression: {}", e);
    }

    let frames = FrameIter::new("videos/input.mp4").map_err(|e| anyhow::anyhow!(e))?;

    // Verify synchronization
    let combined_iter = frames.zip(bounding_boxes.iter());
    
    for (i, (_frame, bbox)) in combined_iter.enumerate().take(10) {
        println!("Processing Frame {}: BBox {:?}", i, bbox);
    }
    
    // In a real application, you might process the rest of the frames here.
    // iteration stops when either frames or bounding_boxes are exhausted.
    
    println!("Total frames processed along with bboxes.");

    Ok(())
}
