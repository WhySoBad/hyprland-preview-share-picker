use std::{io::Read, os::fd::AsFd};

use wayland_client::{
    Dispatch, QueueHandle,
    protocol::{
        wl_buffer::WlBuffer,
        wl_shm::{Format, WlShm},
        wl_shm_pool::WlShmPool,
    },
};

#[derive(Debug)]
pub struct Buffer {
    pub buffer: WlBuffer,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: Format,
    fd: memfd::Memfd,
}

impl Buffer {
    /// create a new buffer to store a single frame
    pub fn new<K: Send + Sync + Clone + 'static, T: Dispatch<WlBuffer, K> + Dispatch<WlShmPool, K> + Dispatch<WlShm, K> + 'static>(
        shm: &WlShm,
        width: u32,
        height: u32,
        stride: u32,
        format: Format,
        handle: &QueueHandle<T>,
        udata: K,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mfd = memfd::MemfdOptions::default().create("buffer")?;
        mfd.as_file().set_len((width * height * 4) as u64)?;
        let pool = shm.create_pool(mfd.as_file().as_fd(), (width * height * 4) as i32, handle, udata.clone());
        let buffer = pool.create_buffer(0, width as i32, height as i32, stride as i32, format, handle, udata);

        pool.destroy();
        Ok(Self { buffer, width, height, stride, format, fd: mfd })
    }

    /// read the bytes from the temporary buffer file
    pub fn get_bytes(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        // let mut file = unsafe { File::from_raw_fd(self.fd) };
        let mut bytes = Vec::new();
        self.fd.as_file().read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// clear the wayland buffer and remove the temporary file
    ///
    /// should only be called after [`get_bytes`] since all data gets deleted by this function
    pub fn destroy(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.buffer.destroy();
        Ok(())
    }
}
