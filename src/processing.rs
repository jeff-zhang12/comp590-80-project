use std::process::Command;
use std::io;

pub fn compress_dynamic_roi(input: &str, output: &str) -> io::Result<()> {
    println!("Starting dynamic differential compression (Two-Pass)...");
    println!("Input: {}", input);
    println!("Output: {}", output);

    let bg_temp = "bg_temp.mp4";

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
    println!("Step 2: Compositing dynamic ROI...");
    
    // Filter Graph:
    // [1:v] is the high-quality input (original).
    // [0:v] is the compressed background.
    // We crop the ROI from [1:v], add a red border, and overlay it on [0:v].
    
    let filter_complex = "
        [1:v] crop=w=iw/2:h=ih/2:x=(iw-ow)/2+sin(t)*(iw-ow)/2:y=(ih-oh)/2,drawbox=c=red:t=10 [roi];
        [0:v][roi] overlay=x=(W-w)/2+sin(t)*(W-w)/2:y=(H-h)/2
    ";
    
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
        .arg("18") // High quality for the composite to preserve the ROI details
        .arg("-c:a")
        .arg("copy") // Copy audio from Input 1
        .arg("-map")
        .arg("1:a?") // Map audio from input 1 if it exists
        .arg(output)
        .status()?;

    // Clean up temp file
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
