mod video;
mod processing;

use std::fs::File;
use std::io::{BufRead, BufReader};
use anyhow::{Context, Result};
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

use std::env;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() >= 2 && args[1] == "baseline" {
        if args.len() != 5 {
            println!("Usage: {} baseline <input_video> <output_video> <crf>", args[0]);
            return Err(anyhow::anyhow!("Invalid arguments for baseline mode"));
        }
        let input_path = &args[2];
        let output_path = &args[3];
        let crf: u8 = args[4].parse().context("CRF must be a number (0-51)")?;

        processing::compress_standard(input_path, output_path, crf)
            .map_err(|e| anyhow::anyhow!("Standard compression failed: {}", e))?;
        
        println!("✓ Baseline compression completed successfully!");
        return Ok(());
    }

    let (input_path, output_path, bbox_path) = if args.len() == 4 {
        (args[1].clone(), args[2].clone(), args[3].clone())
    } else {
        println!("Usage: {} <input_video> <output_video> <bounding_boxes_file>", args[0]);
        println!("       {} baseline <input_video> <output_video> <crf>", args[0]);
        println!("Using default hardcoded paths...");
        (
            "videos/input.mp4".to_string(),
            "videos/output_dynamic.mp4".to_string(),
            "bounding_boxes.txt".to_string(),
        )
    };

    let bounding_boxes = load_bounding_boxes(&bbox_path)?;

    // Run differential compression
    compress_dynamic_roi(&input_path, &output_path, &bounding_boxes)
        .map_err(|e| anyhow::anyhow!("Compression failed: {}", e))?;

    println!("✓ Video compression completed successfully!");

    Ok(())
}
