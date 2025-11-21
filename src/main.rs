use ffmpeg_next as ffmpeg;

fn main() {
    // Initialize FFmpeg
    ffmpeg::init().unwrap();

    // Print FFmpeg version
    let version =  ffmpeg::util::version();
    // let config  = ffmpeg::util::configuration();

    println!("FFmpeg version: {}", version);
    // println!("Config: {}", config);

    let ids = [
        ffmpeg::codec::Id::H264,
        ffmpeg::codec::Id::HEVC,
        ffmpeg::codec::Id::MPEG4,
        ffmpeg::codec::Id::AAC,
        ffmpeg::codec::Id::MP3,
    ];

    // List first 10 codecs
    println!("\nAvailable codecs:");
    for (i, codec) in ids.iter().enumerate() {
        if i >= 10 { break; }

        println!(
            "  {}. {} ",
            i + 1,
            codec.name(),
        );
    }

    println!("\nHello FFmpeg from Rust!");
}
