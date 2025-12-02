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



fn compress_center_roi(input: &str, output: &str) -> std::io::Result<()> {
    println!("Starting differential compression...");
    println!("Input: {}", input);
    println!("Output: {}", output);

    // Region of Interest: Right half of the video (x=iw/2, y=0, w=iw/2, h=ih)
    // This keeps the right half in high quality while the left half is compressed
    let status = Command::new("ffmpeg")
        .arg("-y") // Overwrite output file
        .arg("-i")
        .arg(input)
        .arg("-vf")
        .arg("addroi=x=iw/2:y=0:w=iw/2:h=ih:qoffset=-1.0")
        .arg("-c:v")
        .arg("libx264")
        .arg("-crf")
        .arg("51") // Max compression for the background
        .arg(output)
        .status()?;

    if status.success() {
        println!("Compression completed successfully.");
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "FFmpeg command failed",
        ))
    }
}

fn main() -> std::io::Result<()> {
    // Initialize FFmpeg
    // ffmpeg::init().unwrap();

    // Run differential compression
    if let Err(e) = compress_center_roi("input.mp4", "output_roi.mp4") {
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
