use std::rc::Rc;

use image::{imageops::{flip_vertical_in_place, resize, rotate180_in_place, rotate270, rotate90}, RgbImage, RgbaImage};

use crate::buffer::Buffer;

/// Xrgb8888 buffered image (as returned by hyprland) stored as a rgba image
pub type XrgbImage = RgbaImage;

pub enum ImageKind {
    Rgb(RgbImage),
    Xrgb(XrgbImage),
}

pub struct Image {
    pub buffer: ImageKind,
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

    /// apply an output transformation to the image
    pub fn transform(mut self, transform: Transforms) -> Self {
        self.buffer = match transform {
            Transforms::Normal => self.buffer,
            Transforms::Normal90 => match self.buffer {
                ImageKind::Rgb(image_buffer) => ImageKind::Rgb(rotate90(&image_buffer)),
                ImageKind::Xrgb(image_buffer) => ImageKind::Xrgb(rotate90(&image_buffer)),
            },
            Transforms::Normal180 => {
                match &mut self.buffer {
                    ImageKind::Rgb(image_buffer) => rotate180_in_place(image_buffer),
                    ImageKind::Xrgb(image_buffer) => rotate180_in_place(image_buffer),
                };
                self.buffer
            }
            Transforms::Normal270 => match self.buffer {
                ImageKind::Rgb(image_buffer) => ImageKind::Rgb(rotate270(&image_buffer)),
                ImageKind::Xrgb(image_buffer) => ImageKind::Xrgb(rotate270(&image_buffer)),
            },
            Transforms::Flipped => {
                match &mut self.buffer {
                    ImageKind::Rgb(image_buffer) => flip_vertical_in_place(image_buffer),
                    ImageKind::Xrgb(image_buffer) => flip_vertical_in_place(image_buffer),
                }
                self.buffer
            }
            Transforms::Flipped90 => match &mut self.buffer {
                ImageKind::Rgb(image_buffer) => {
                    flip_vertical_in_place(image_buffer);
                    ImageKind::Rgb(rotate90(image_buffer))
                },
                ImageKind::Xrgb(image_buffer) => {
                    flip_vertical_in_place(image_buffer);
                    ImageKind::Xrgb(rotate90(image_buffer))
                },
            },
            Transforms::Flipped180 => {
                match &mut self.buffer {
                    ImageKind::Rgb(image_buffer) => {
                        flip_vertical_in_place(image_buffer);
                        rotate180_in_place(image_buffer);
                    },
                    ImageKind::Xrgb(image_buffer) => {
                        flip_vertical_in_place(image_buffer);
                        rotate180_in_place(image_buffer);
                    }
                };
                self.buffer
            },
            Transforms::Flipped270 => match &mut self.buffer {
                ImageKind::Rgb(image_buffer) => {
                    flip_vertical_in_place(image_buffer);
                    ImageKind::Rgb(rotate270(image_buffer))
                },
                ImageKind::Xrgb(image_buffer) => {
                    flip_vertical_in_place(image_buffer);
                    ImageKind::Xrgb(rotate270(image_buffer))
                },
            },
        };

        self.aspect_ratio = match &self.buffer {
            ImageKind::Rgb(image_buffer) => image_buffer.width() as f64 / image_buffer.height() as f64,
            ImageKind::Xrgb(image_buffer) => image_buffer.width() as f64 / image_buffer.height() as f64,
        };
        self
    }

    /// resize the image buffer such that the bigger of the two dimensions is `size` long
    pub fn resize_to_fit(&mut self, size: u32) {
        let (width, height) = match &self.buffer {
            ImageKind::Rgb(image_buffer) => (image_buffer.width(), image_buffer.height()),
            ImageKind::Xrgb(image_buffer) => (image_buffer.width(), image_buffer.height()),
        };
        if height > width && width > size {
            let height = (size as f64 / self.aspect_ratio) as u32;
            self.resize(size, height);
        } else if width > height && height > size {
            let width = (size as f64 * self.aspect_ratio) as u32;
            self.resize(width, size);
        }
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
}

pub enum Transforms {
    Normal,
    Normal90,
    Normal180,
    Normal270,
    Flipped,
    Flipped90,
    Flipped180,
    Flipped270,
}

#[cfg(feature = "hyprland-rs")]
impl From<hyprland::data::Transforms> for Transforms {
    fn from(value: hyprland::data::Transforms) -> Self {
        match value {
            hyprland::data::Transforms::Normal => Transforms::Normal,
            hyprland::data::Transforms::Normal90 => Transforms::Normal90,
            hyprland::data::Transforms::Normal180 => Transforms::Normal180,
            hyprland::data::Transforms::Normal270 => Transforms::Normal270,
            hyprland::data::Transforms::Flipped => Transforms::Flipped,
            hyprland::data::Transforms::Flipped90 => Transforms::Flipped90,
            hyprland::data::Transforms::Flipped180 => Transforms::Flipped180,
            hyprland::data::Transforms::Flipped270 => Transforms::Flipped270,
        }
    }
}