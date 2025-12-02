use ffmpeg_next as ffmpeg;
use ffmpeg::{
    codec,
    format,
    frame,
    media::Type,
    software::scaling,
    util::frame::video::Video,
};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

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
    
    pub fn stream_params(&self) -> codec::Parameters {
        self.ictx.stream(self.video_stream_index).unwrap().parameters()
    }
    
    pub fn time_base(&self) -> ffmpeg::Rational {
        self.ictx.stream(self.video_stream_index).unwrap().time_base()
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

    let input_file = "videos/input.mp4";
    let output_file = "videos/output.avi";
    let bbox_file = "bounding_boxes_dummy.txt"; // Use dummy for now, or check for real one

    let mut frames_iter = FrameIter::new(input_file)?;
    
    // Setup Output
    let mut octx = format::output(&output_file)?;
    
    // Create fresh encoder context
    let mut encoder = codec::context::Context::new();
    let mut encoder = encoder.encoder().video()?;
    
    // Configure encoder
    encoder.set_height(frames_iter.decoder.height());
    encoder.set_width(frames_iter.decoder.width());
    encoder.set_aspect_ratio(frames_iter.decoder.aspect_ratio());
    encoder.set_format(frames_iter.decoder.format());
    
    let frame_rate = frames_iter.decoder.frame_rate().unwrap_or(ffmpeg::Rational(30, 1));
    encoder.set_frame_rate(Some(frame_rate));
    encoder.set_time_base(frame_rate.invert());
    encoder.set_flags(codec::Flags::GLOBAL_HEADER);
    encoder.set_max_b_frames(0); // Disable B-frames to simplify timestamps
    encoder.set_bit_rate(1_000_000); // 1 Mbps
    
    println!("Encoder settings: {}x{}, fmt: {:?}, rate: {}, tb: {}", 
             encoder.width(), encoder.height(), encoder.format(), frame_rate, encoder.time_base());
    
    let mut encoder = encoder.open_as(codec::Id::MPEG4)?;

    let ost_index = {
        let mut ost = octx.add_stream(ffmpeg::encoder::find(codec::Id::MPEG4))?;
        ost.set_parameters(&encoder);
        ost.index()
    };
    
    octx.write_header()?;

    // Load bounding boxes
    let mut bounding_boxes: Vec<(i32, i32, i32, i32)> = Vec::new();
    let bbox_path = if Path::new("bounding_boxes.txt").exists() {
        "bounding_boxes.txt"
    } else {
        bbox_file
    };

    if let Ok(file) = File::open(bbox_path) {
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
        eprintln!("could not open bounding boxes file: {}", bbox_path);
    }

    let mut frame_count = 0;

    // Process frames
    for (i, mut frame) in frames_iter.enumerate() {
        // Get bounding box for this frame
        let bbox = if i < bounding_boxes.len() {
            bounding_boxes[i]
        } else {
             *bounding_boxes.last().unwrap_or(&(0,0,0,0))
        };
        
        let (bx1, by1, bx2, by2) = bbox;

        let width = frame.width() as i32;
        let height = frame.height() as i32;
        
        // Y Plane
        {
            let stride_y = frame.stride(0);
            let data_y = frame.data_mut(0);
            
            for y in 0..height {
                for x in 0..width {
                    if x < bx1 || x > bx2 || y < by1 || y > by2 {
                        let idx = (y as usize) * stride_y + (x as usize);
                        data_y[idx] = 0;
                    }
                }
            }
        }
        
        // U Plane
        let stride_u = frame.stride(1);
        let uv_width = width / 2;
        let uv_height = height / 2;
        let uv_bx1 = bx1 / 2;
        let uv_bx2 = bx2 / 2;
        let uv_by1 = by1 / 2;
        let uv_by2 = by2 / 2;

        {
            let data_u = frame.data_mut(1);
            for y in 0..uv_height {
                for x in 0..uv_width {
                     if x < uv_bx1 || x > uv_bx2 || y < uv_by1 || y > uv_by2 {
                        let idx_u = (y as usize) * stride_u + (x as usize);
                        data_u[idx_u] = 128;
                     }
                }
            }
        }

        // V Plane
        let stride_v = frame.stride(2);
        {
            let data_v = frame.data_mut(2);
            for y in 0..uv_height {
                for x in 0..uv_width {
                     if x < uv_bx1 || x > uv_bx2 || y < uv_by1 || y > uv_by2 {
                        let idx_v = (y as usize) * stride_v + (x as usize);
                        data_v[idx_v] = 128;
                     }
                }
            }
        }

        // Encode and write
        frame.set_pts(Some(i as i64)); // Simple PTS
        
        send_frame_to_encoder(&mut encoder, &frame, &mut octx, ost_index)?;
        frame_count += 1;
    }
    
    // Flush encoder
    encoder.send_eof()?;
    receive_and_write_packets(&mut encoder, &mut octx, ost_index)?;

    octx.write_trailer()?;
    
    println!("Processed {} frames.", frame_count);

    Ok(())
}

fn send_frame_to_encoder(
    encoder: &mut codec::encoder::Video,
    frame: &frame::Video,
    octx: &mut format::context::Output,
    stream_index: usize,
) -> Result<(), ffmpeg::Error> {
    encoder.send_frame(frame)?;
    receive_and_write_packets(encoder, octx, stream_index)
}

fn receive_and_write_packets(
    encoder: &mut codec::encoder::Video,
    octx: &mut format::context::Output,
    stream_index: usize,
) -> Result<(), ffmpeg::Error> {
    let mut packet = ffmpeg::Packet::empty();
    while encoder.receive_packet(&mut packet).is_ok() {
        packet.set_stream(stream_index);
        
        let pts_before = packet.pts();
        let dts_before = packet.dts();
        packet.rescale_ts(encoder.time_base(), octx.stream(stream_index).unwrap().time_base());
        let pts_after = packet.pts();
        let dts_after = packet.dts();
        
        println!("Packet PTS: {:?} -> {:?}, DTS: {:?} -> {:?}", pts_before, pts_after, dts_before, dts_after);
        
        packet.write_interleaved(octx)?;
    }
    Ok(())
}
