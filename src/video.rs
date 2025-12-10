use ffmpeg_next as ffmpeg;
use ffmpeg::{
    codec,
    format,
    frame,
    media::Type,
    software::scaling::{context::Context as ScalingContext, flag::Flags},
    util::format::pixel::Pixel,
};

pub struct FrameIter {
    ictx: format::context::Input,
    decoder: codec::decoder::Video,
    scaler: ScalingContext,
    video_stream_index: usize,
    finished: bool,
    fps: f64,
}

impl FrameIter {
    pub fn new(path: &str) -> Result<Self, ffmpeg::Error> {
        ffmpeg::init()?;

        eprintln!("Opening video file: {}", path);
        let ictx = format::input(&path)?;
        
        eprintln!("Finding video stream...");
        let input = ictx
            .streams()
            .best(Type::Video)
            .ok_or(ffmpeg::Error::StreamNotFound)?;
        let video_stream_index = input.index();
        eprintln!("Video stream index: {}", video_stream_index);

        eprintln!("Creating decoder...");
        let context_decoder = codec::Context::from_parameters(input.parameters())?;
        let decoder = context_decoder.decoder().video()?;

        let width = decoder.width();
        let height = decoder.height();
        eprintln!("Video dimensions: {}x{}", width, height);
        eprintln!("Video format: {:?}", decoder.format());

        // Create scaler to convert to RGB24 for easier processing
        eprintln!("Creating scaler...");
        let scaler = ScalingContext::get(
            decoder.format(),
            decoder.width(),
            decoder.height(),
            Pixel::RGB24,
            width,
            height,
            Flags::FAST_BILINEAR,
        )?;
        eprintln!("FrameIter initialized successfully");

        let fps = input.rate();
        let fps_f64 = fps.numerator() as f64 / fps.denominator() as f64;
        eprintln!("Video FPS: {:.2}", fps_f64);

        Ok(Self {
            ictx,
            decoder,
            scaler,
            video_stream_index,
            finished: false,
            fps: fps_f64,
        })
    }

    pub fn fps(&self) -> f64 {
        self.fps
    }
}

impl Iterator for FrameIter {
    type Item = frame::Video;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        let mut decoded = frame::Video::empty();
        let mut rgb_frame = frame::Video::empty();

        let mut packet_count = 0;
        let max_packets = 1000; // Prevent infinite loop

        loop {
            packet_count += 1;
            if packet_count > max_packets {
                eprintln!("ERROR: Processed too many packets without getting a frame");
                self.finished = true;
                return None;
            }

            // Try to receive a frame from decoder
            match self.decoder.receive_frame(&mut decoded) {
                Ok(_) => {
                    // Convert to RGB24
                    match self.scaler.run(&decoded, &mut rgb_frame) {
                        Ok(_) => {
                            return Some(rgb_frame);
                        }
                        Err(e) => {
                            eprintln!("Frame scaling error: {:?}", e);
                            return Some(decoded); // Fallback to original frame
                        }
                    }
                }
                Err(ffmpeg::Error::Eof) => {
                    self.finished = true;
                    return None;
                }
                Err(ffmpeg::Error::Other { errno: 11 }) | Err(ffmpeg::Error::Other { errno: -11 }) => {
                    // EAGAIN - need more data, continue to send packets
                }
                Err(e) => {
                    eprintln!("Unexpected decoder error: {:?}", e);
                    self.finished = true;
                    return None;
                }
            }

            // Send packets to decoder
            match self.ictx.packets().next() {
                Some((stream, packet)) if stream.index() == self.video_stream_index => {
                    if let Err(e) = self.decoder.send_packet(&packet) {
                        eprintln!("send_packet error: {:?}", e);
                        self.finished = true;
                        return None;
                    }
                }
                Some(_) => {
                    // Skip non-video packets
                    continue;
                }
                None => {
                    // End of input - flush decoder
                    let _ = self.decoder.send_eof();
                    
                    // Try one more time to get remaining frames
                    match self.decoder.receive_frame(&mut decoded) {
                        Ok(_) => {
                            if self.scaler.run(&decoded, &mut rgb_frame).is_ok() {
                                return Some(rgb_frame);
                            }
                        }
                        _ => {}
                    }
                    
                    self.finished = true;
                    return None;
                }
            }
        }
    }
}
