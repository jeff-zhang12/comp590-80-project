// use ffmpeg_next as ffmpeg;
// use ffmpeg::{
//     codec,
//     format,
//     frame,
//     media::Type,
// };
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::process::Command;

/*
pub struct FrameIter {
    ictx: format::context::Input,
    decoder: codec::decoder::Video,
    video_stream_index: usize,
    finished: bool,
}

impl FrameIter {
    pub fn new(path: &str) -> Result<Self, ffmpeg::Error> {
        ffmpeg::init()?;

        let ictx = format::input(&path)?;
        let input = ictx
            .streams()
            .best(Type::Video)
            .ok_or(ffmpeg::Error::StreamNotFound)?;
        let video_stream_index = input.index();

        let context_decoder = codec::Context::from_parameters(input.parameters())?;
        let decoder = context_decoder.decoder().video()?;

        Ok(Self {
            ictx,
            decoder,
            video_stream_index,
            finished: false,
        })
    }
}

impl Iterator for FrameIter {
    type Item = frame::Video;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        let mut decoded = frame::Video::empty();

        loop {
            if self.decoder.receive_frame(&mut decoded).is_ok() {
                return Some(decoded);
            }

            match self.ictx.packets().next() {
                Some((stream, packet)) if stream.index() == self.video_stream_index => {
                    if let Err(e) = self.decoder.send_packet(&packet) {
                        eprintln!("send_packet error: {:?}", e);
                        self.finished = true;
                        return None;
                    }
                }
                Some(_) => {
                    continue;
                }
                None => {
                    if let Err(e) = self.decoder.send_eof() {
                        eprintln!("send_eof error: {:?}", e);
                    }
                    if self.decoder.receive_frame(&mut decoded).is_ok() {
                        return Some(decoded);
                    }
                    self.finished = true;
                    return None;
                }
            }
        }
    }
}
*/



fn load_bounding_boxes(path: &str) -> std::io::Result<Vec<(i32, i32, i32, i32)>> {
    let mut bounding_boxes = Vec::new();
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    
    for line_result in reader.lines() {
        if let Ok(line) = line_result {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split(',').collect();
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
    }
    
    Ok(bounding_boxes)
}

fn compress_dynamic_roi(input: &str, output: &str, bbox_file: &str, fps: f64) -> std::io::Result<()> {
    println!("Starting dynamic differential compression (Two-Pass)...");
    println!("Input: {}", input);
    println!("Output: {}", output);

    // Load bounding boxes
    println!("Loading bounding boxes from {}...", bbox_file);
    let bounding_boxes = load_bounding_boxes(bbox_file)?;
    println!("Loaded {} bounding boxes", bounding_boxes.len());

    if bounding_boxes.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "No bounding boxes found",
        ));
    }

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
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "FFmpeg background generation failed",
        ));
    }

    // Step 2: Process frame by frame with bounding boxes
    println!("Step 2: Processing frames with bounding boxes...");
    
    // Create a more complex approach: extract frames, process each, then re-encode
    // This is more reliable than trying to use sendcmd for this complex operation
    
    let frames_dir = "temp_frames";
    let roi_frames_dir = "temp_roi_frames";
    let output_frames_dir = "temp_output_frames";
    
    // Create directories
    std::fs::create_dir_all(frames_dir)?;
    std::fs::create_dir_all(roi_frames_dir)?;
    std::fs::create_dir_all(output_frames_dir)?;
    
    // Extract frames from compressed background
    println!("  Extracting background frames...");
    let status_extract_bg = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(bg_temp)
        .arg(format!("{}/frame_%06d.png", frames_dir))
        .status()?;
    
    if !status_extract_bg.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to extract background frames",
        ));
    }
    
    // Extract frames from original video
    println!("  Extracting original frames...");
    let status_extract_orig = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(input)
        .arg(format!("{}/frame_%06d.png", roi_frames_dir))
        .status()?;
    
    if !status_extract_orig.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to extract original frames",
        ));
    }
    
    // Process each frame
    println!("  Compositing frames with ROI...");
    for (i, bbox) in bounding_boxes.iter().enumerate() {
        let frame_num = i + 1;
        let (x1, y1, x2, y2) = bbox;
        let width = x2 - x1;
        let height = y2 - y1;
        
        // Ensure valid dimensions
        if width <= 0 || height <= 0 {
            eprintln!("Warning: Invalid bounding box at frame {}: {:?}", frame_num, bbox);
            continue;
        }
        
        let bg_frame = format!("{}/frame_{:06}.png", frames_dir, frame_num);
        let roi_frame = format!("{}/frame_{:06}.png", roi_frames_dir, frame_num);
        let output_frame = format!("{}/frame_{:06}.png", output_frames_dir, frame_num);
        
        // Check if files exist
        if !std::path::Path::new(&bg_frame).exists() || !std::path::Path::new(&roi_frame).exists() {
            eprintln!("Warning: Missing frame {}", frame_num);
            break;
        }
        
        // Crop ROI from original and overlay on background
        let filter = format!(
            "[1:v]crop={}:{}:{}:{},drawbox=c=red:t=5[roi];[0:v][roi]overlay={}:{}",
            width, height, x1, y1, x1, y1
        );
        
        let status_composite = Command::new("ffmpeg")
            .arg("-y")
            .arg("-i")
            .arg(&bg_frame)
            .arg("-i")
            .arg(&roi_frame)
            .arg("-filter_complex")
            .arg(&filter)
            .arg(&output_frame)
            .status()?;
        
        if !status_composite.success() {
            eprintln!("Warning: Failed to composite frame {}", frame_num);
        }
        
        if frame_num % 30 == 0 {
            println!("    Processed {} frames...", frame_num);
        }
    }
    
    // Re-encode video from frames
    println!("Step 3: Re-encoding final video...");
    let status_encode = Command::new("ffmpeg")
        .arg("-y")
        .arg("-framerate")
        .arg(fps.to_string())
        .arg("-i")
        .arg(format!("{}/frame_%06d.png", output_frames_dir))
        .arg("-i")
        .arg(input)  // For audio
        .arg("-c:v")
        .arg("libx264")
        .arg("-crf")
        .arg("18")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-c:a")
        .arg("copy")
        .arg("-map")
        .arg("0:v")
        .arg("-map")
        .arg("1:a?")
        .arg("-shortest")
        .arg(output)
        .status()?;
    
    // Clean up temp files
    println!("Cleaning up temporary files...");
    let _ = std::fs::remove_file(bg_temp);
    let _ = std::fs::remove_dir_all(frames_dir);
    let _ = std::fs::remove_dir_all(roi_frames_dir);
    let _ = std::fs::remove_dir_all(output_frames_dir);

    if status_encode.success() {
        println!("Compression completed successfully.");
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "FFmpeg final encoding failed",
        ))
    }
}

fn main() -> std::io::Result<()> {
    // Initialize FFmpeg
    // ffmpeg::init().unwrap();

    // Run differential compression with bounding box tracking
    // Adjust FPS to match your video (common values: 24, 25, 29.97, 30, 60)
    let fps = 30.0;  // Change this to match your video's FPS
    
    if let Err(e) = compress_dynamic_roi("input.mp4", "output_dynamic.mp4", "bounding_boxes.txt", fps) {
        eprintln!("Error during compression: {}", e);
    }

    /*
    let frames = FrameIter::new("videos/input.mp4")?;

    // Load bounding boxes written by mapper.py (x1,y1,x2,y2 per line)
    let mut bounding_boxes: Vec<(i32, i32, i32, i32)> = Vec::new();

	if let Ok(file) = File::open("bounding_boxes.txt") {
		let reader = BufReader::new(file);
		for line_result in reader.lines() {
			if let Ok(line) = line_result {
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
		}
	} else {
		eprintln!("could not open bounding_boxes.txt");
	}


    println!("Bounding boxes: {:?}", bounding_boxes);
    println!("Total frames: {}", frames.count());
    */

    Ok(())
}
