use std::sync::Arc;

use buffer::Buffer;
use frame::FrameManager;
use output::OutputManager;
use wayland_client::Connection;

mod protocols;
mod buffer;
mod output;
mod frame;

const WINDOW_HANDLE: u64 = 0x5c458aecf480;
const WINDOW_HANDLE_2: u64 = 0x5c458af27070;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let con = Connection::connect_to_env()?;

    let mut manager = OutputManager::new(&con)?;

    manager.outputs.clone().into_iter().for_each(|(wl_output, output)| {
        let buffer = manager.capture_output_region(&wl_output, -1000,0, 3000, 250).expect("should capture output");
        if let Some(name) = output.name {
            write_image(buffer, format!("{name}.png").as_str());
        }
    });

    // let mut manager = FrameManager::new(&con)?;
    // let frame_buffer = manager.get_frame(WINDOW_HANDLE)?;
    // write_image(frame_buffer.clone(), "first.png");
    // frame_buffer.destroy()?;

    // let frame_buffer = manager.get_frame(WINDOW_HANDLE_2)?;
    // write_image(frame_buffer.clone(), "second.png");
    // frame_buffer.destroy()?;

    // manager.destroy();
    Ok(())
}

fn write_image(buffer: Arc<Buffer>, name: &str) {
    let mut raw_bytes = buffer.get_bytes().expect("should get bytes");

    println!("starting to flip channels");
    for i in 0..(raw_bytes.len() / 4) {
        let offset = i * 4;
        let b = raw_bytes[offset];
        let g = raw_bytes[offset + 1];
        let r = raw_bytes[offset + 2];
        let a = raw_bytes[offset + 3];
        raw_bytes[offset] = r;
        raw_bytes[offset + 1] = g;
        raw_bytes[offset + 2] = b;
        raw_bytes[offset + 3] = a;
    }
    println!("finished flipping channels");

    let img = image::RgbaImage::from_vec(
        buffer.width,
        buffer.height,
        raw_bytes
    ).expect("should create rgba image");
    println!("created image");
    img.save(name).expect("should save png");
    println!("saved image");
}