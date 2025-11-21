use ffmpeg_next as ffmpeg;

fn main() {
    // Initialize FFmpeg
    ffmpeg::init().unwrap();

    // Print FFmpeg version
    let version = ffmpeg::util::version::version();
    let config  = ffmpeg::util::version::configuration();

    println!("FFmpeg version: {}", version);
    println!("Config: {}", config);

    // List first 10 codecs
    println!("\nAvailable codecs:");
    for (i, codec) in ffmpeg::codec::all().enumerate() {
        if i >= 10 { break; }

        println!(
            "  {}. {} ({})",
            i + 1,
            codec.name(),
            codec.medium()
        );
    }

    println!("\nHello FFmpeg from Rust!");
}
