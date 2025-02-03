use std::{fs::{self, File}, io::{BufWriter, Write}, os::fd::AsFd, path::PathBuf};

use uuid::Uuid;
use wayland_client::{protocol::{wl_buffer::WlBuffer, wl_shm::{Format, WlShm}}, QueueHandle};

use crate::AppData;

pub struct Buffer {
    pub path: PathBuf,
    pub buffer: WlBuffer,
    pub width: u32,
    pub height: u32,
    pub format: Format,
}

impl Buffer {
    /// create a new buffer to store a single frame
    pub fn new(shm: &WlShm, width: u32, height: u32, stride: u32, format: Format, handle: &QueueHandle<AppData>) -> Result<Self, Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!("hyprland-screen-picker-{}", Uuid::new_v4().to_string()));
        let file = File::options()
            .create(true)
            .write(true)
            .read(true)
            .truncate(true)
            .open(&path)?;

        let mut writer = BufWriter::new(&file);
        let data = vec![0_u8; (width * height * 4) as usize];
        writer.write_all(data.as_slice())?;
        writer.flush()?;

        let fd = file.as_fd();
        let pool = shm.create_pool(fd, (width * height * 4) as i32, handle, ());
        let buffer = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            stride as i32,
            format,
            handle,
            ()
        );

        pool.destroy();

        Ok(Self { path, buffer, width, height, format })
    }

    /// read the bytes from the temporary buffer file
    pub fn get_bytes(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        return Ok(fs::read(&self.path)?);
    }

    /// clear the wayland buffer and remove the temporary file
    ///
    /// should only be called after [`get_bytes`] since all data gets deleted by this function
    pub fn destroy(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.buffer.destroy();
        if self.path.exists() {
            fs::remove_file(&self.path)?
        }

        Ok(())
    }
}