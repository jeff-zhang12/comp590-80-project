use ffmpeg_next as ffmpeg;
use ffmpeg::{
    codec,
    format,
    frame,
    media::Type,
};
use std::fs::File;
use std::io::{BufRead, BufReader};

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


fn main() -> Result<(), ffmpeg::Error> {
    // Initialize FFmpeg
    ffmpeg::init().unwrap();

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

    Ok(())
}
