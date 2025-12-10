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

        let width = decoder.width();
        let height = decoder.height();

        // Create scaler to convert to RGB24 for easier processing
        let scaler = ScalingContext::get(
            decoder.format(),
            decoder.width(),
            decoder.height(),
            Pixel::RGB24,
            width,
            height,
            Flags::BILINEAR,
        )?;

        Ok(Self {
            ictx,
            decoder,
            scaler,
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
        let mut rgb_frame = frame::Video::empty();

        loop {
            if self.decoder.receive_frame(&mut decoded).is_ok() {
                // Convert to RGB24
                if self.scaler.run(&decoded, &mut rgb_frame).is_ok() {
                    return Some(rgb_frame);
                } else {
                    eprintln!("Frame scaling error");
                    return Some(decoded); // Fallback to original frame
                }
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
                    if let Err(_e) = self.decoder.send_eof() {
                        // ignore EOF error
                    }
                    if self.decoder.receive_frame(&mut decoded).is_ok() {
                        if self.scaler.run(&decoded, &mut rgb_frame).is_ok() {
                            return Some(rgb_frame);
                        }
                    }
                    self.finished = true;
                    return None;
                }
            }
        }
    }
}
