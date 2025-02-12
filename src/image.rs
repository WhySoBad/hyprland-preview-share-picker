use gtk4::gdk_pixbuf::Pixbuf;
use hyprland_preview_share_picker_lib::image::{Image, ImageKind};

pub trait ImageExt {
    /// turn the image into a gdk pixbuf which can directly be displayed inside a gtk image
    fn into_pixbuf(self) -> Result<Pixbuf, Box<dyn std::error::Error>>;
}

impl ImageExt for Image {
    fn into_pixbuf(self) -> Result<Pixbuf, Box<dyn std::error::Error>> {
        let rgb_image = match self.into_rgb()?.buffer {
            ImageKind::Xrgb(_) => unreachable!("the image just got converted to rgb"),
            ImageKind::Rgb(image_buffer) => image_buffer,
        };

        let height = rgb_image.height() as i32;
        let width = rgb_image.width() as i32;

        let bytes = gtk4::glib::Bytes::from(&rgb_image.into_vec());
        let pixbuf = Pixbuf::from_bytes(&bytes, gtk4::gdk_pixbuf::Colorspace::Rgb, false, 8, width, height, width * 3);
        Ok(pixbuf)
    }
}
