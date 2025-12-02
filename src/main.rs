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



fn compress_dynamic_roi(input: &str, output: &str) -> std::io::Result<()> {
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
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
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
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "FFmpeg composition failed",
        ))
    }
}

fn main() -> std::io::Result<()> {
    // Initialize FFmpeg
    // ffmpeg::init().unwrap();

    // Run differential compression
    if let Err(e) = compress_dynamic_roi("input.mp4", "output_dynamic.mp4") {
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
