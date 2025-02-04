use std::rc::Rc;

use gtk4::gdk_pixbuf::Pixbuf;
use hyprland_screen_picker_protocols::buffer::Buffer;
use image::{RgbImage, RgbaImage, imageops::resize};

/// Xrgb8888 buffered image (as returned by hyprland) stored as a rgba image
type XrgbImage = RgbaImage;

enum ImageKind {
    Rgb(RgbImage),
    Xrgb(XrgbImage),
}

pub struct Image {
    buffer: ImageKind,
    pub aspect_ratio: f64,
}

impl Image {
    /// create a new image from a buffer storing a frame
    pub fn new(buffer: Rc<Buffer>) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = buffer.get_bytes()?;
        buffer.destroy()?;
        let img = match XrgbImage::from_vec(buffer.width, buffer.height, bytes) {
            Some(img) => Self { buffer: ImageKind::Xrgb(img), aspect_ratio: buffer.width as f64 / buffer.height as f64 },
            None => return Err(Box::from("failed to create xrgb image from buffer")),
        };
        drop(buffer);
        Ok(img)
    }

    /// resize the image buffer to the specified dimensions
    pub fn resize(&mut self, width: u32, height: u32) {
        match &self.buffer {
            ImageKind::Rgb(image_buffer) => {
                let sized = resize(image_buffer, width, height, image::imageops::FilterType::Triangle);
                self.buffer = ImageKind::Rgb(sized);
            }
            ImageKind::Xrgb(image_buffer) => {
                let sized = resize(image_buffer, width, height, image::imageops::FilterType::Triangle);
                self.buffer = ImageKind::Xrgb(sized);
            }
        }
    }

    /// resize the image buffer such that the bigger of the two dimensions is `size` long
    pub fn resize_to_fit(&mut self, size: u32) {
        let (width, height) = if self.aspect_ratio > 1.0 {
            (size, (size as f64 / self.aspect_ratio) as u32)
        } else {
            ((size as f64 / self.aspect_ratio) as u32, size)
        };
        self.resize(width, height);
    }

    /// convert a possible xrgb image instance into a rgb image instance
    ///
    /// if the instance is already a rgb instance nothing happens
    pub fn into_rgb(self) -> Result<Self, Box<dyn std::error::Error>> {
        let ImageKind::Xrgb(xrgb_buffer) = self.buffer else {
            return Ok(self);
        };
        let aspect_ratio = self.aspect_ratio;

        Ok(Self { buffer: ImageKind::Rgb(Self::convert_xrgb_to_rgb(xrgb_buffer)?), aspect_ratio })
    }

    /// convert a xrgb buffer into a rgb buffer
    fn convert_xrgb_to_rgb(buffer: XrgbImage) -> Result<RgbImage, Box<dyn std::error::Error>> {
        let height = buffer.height();
        let width = buffer.width();

        let raw = buffer.into_vec();
        let bytes = raw.into_iter().array_chunks::<4>().flat_map(|[b, g, r, _]| [r, g, b]).collect::<Vec<_>>();
        match RgbImage::from_vec(width, height, bytes) {
            Some(img) => Ok(img),
            None => Err(Box::from("failed to convert xrgb image to rgb image")),
        }
    }

    /// turn the image into a gdk pixbuf which can directly be displayed inside a gtk image
    pub fn into_pixbuf(self) -> Result<Pixbuf, Box<dyn std::error::Error>> {
        let rgb_image = match self.buffer {
            ImageKind::Xrgb(image_buffer) => Self::convert_xrgb_to_rgb(image_buffer)?,
            ImageKind::Rgb(image_buffer) => image_buffer,
        };

        let height = rgb_image.height() as i32;
        let width = rgb_image.width() as i32;

        let bytes = gtk4::glib::Bytes::from(&rgb_image.into_vec());
        let pixbuf = Pixbuf::from_bytes(&bytes, gtk4::gdk_pixbuf::Colorspace::Rgb, false, 8, width, height, width * 3);
        Ok(pixbuf)
    }
}
