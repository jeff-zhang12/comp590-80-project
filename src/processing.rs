use std::process::Command;
use std::io::{self, Write};
use std::fs::File;

pub fn compress_dynamic_roi(input: &str, output: &str, bboxes: &[(i32, i32, i32, i32)]) -> io::Result<()> {
    println!("Starting dynamic differential compression (Two-Pass) with Bounding Boxes...");
    println!("Input: {}", input);
    println!("Output: {}", output);

    let bg_temp = "bg_temp.mp4";
    let filter_script_path = "filter_script.txt";

    // Step 1: Create heavily compressed background
    println!("Step 1: Generating compressed background...");
    let status_bg = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(input)
        .arg("-c:v")
        .arg("libx264")
        .arg("-crf")
        .arg("51") // Max compression (worst quality)
        .arg("-an") // No audio for background temp
        .arg(bg_temp)
        .status()?;

    if !status_bg.success() {
        return Err(io::Error::other(
            "FFmpeg background generation failed",
        ));
    }

    // Step 2: Overlay high-quality moving ROI on top of compressed background
    println!("Step 2: Generaling filter script and compositing...");

    // Generate chained drawbox filters
    // Start with a black box covering the whole frame
    let mut drawbox_chain = String::from("[1:v]drawbox=t=fill:c=black");

    // Loop through bboxes and append drawbox filter for each frame
    for (i, (x1, y1, x2, y2)) in bboxes.iter().enumerate() {
        // Macroblock alignment (16x16 grid)
        // Round start down to nearest multiple of 16
        // Round end up to nearest multiple of 16
        let x1_aligned = (x1 / 16) * 16;
        let y1_aligned = (y1 / 16) * 16;
        let x2_aligned = ((x2 + 15) / 16) * 16;
        let y2_aligned = ((y2 + 15) / 16) * 16;

        let w = x2_aligned - x1_aligned;
        let h = y2_aligned - y1_aligned;
        
        // Append drawbox for this specific frame
        // enable='eq(n\,i)' ensures it only draws on frame i
        // We accumulate these filters in a chain.
        // Warning: for many frames, this chain is long, but likely handled better than one huge expression.
        let filter = format!(",drawbox=x={}:y={}:w={}:h={}:t=fill:c=white:enable='eq(n\\,{})'", x1_aligned, y1_aligned, w, h, i);
        drawbox_chain.push_str(&filter);
    }
    
    // Terminate the chain and label output [mask]
    drawbox_chain.push_str("[mask];");
    
    // Filter Graph:
    // [1:v] is the HQ original.
    // [drawbox_chain] creates the mask (white ROI on black).
    // alphamerge applies mask to [1:v] copy.
    // overlay puts it on compressed background.
    
    let filter_complex = format!(
        "{}[1:v][mask]alphamerge[fg];[0:v][fg]overlay=0:0:shortest=1",
        drawbox_chain
    );

    let status_final = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(bg_temp)   // Input 0: Compressed BG
        .arg("-i")
        .arg(input)     // Input 1: High Quality Original
        .arg("-filter_complex")
        .arg(filter_complex)
        .arg("-c:v")
        .arg("libx264")
        .arg("-crf")
        .arg("18") // High quality for the composite
        .arg("-c:a")
        .arg("copy")
        .arg("-map")
        .arg("1:a?")
        .arg(output)
        .status()?;

    // Clean up temp files
    let _ = std::fs::remove_file(bg_temp);

    if status_final.success() {
        println!("Compression completed successfully.");
        Ok(())
    } else {
        Err(io::Error::other(
            "FFmpeg composition failed",
        ))
    }
}
