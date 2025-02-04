use gtk::{Application, ApplicationWindow, gio::prelude::ApplicationExtManual, glib::ExitCode};
use gtk4::{
    self as gtk, Box, FlowBox, Image as GtkImage, Label, ScrolledWindow, Widget,
    gio::prelude::ApplicationExt,
    glib::object::IsA,
    prelude::{BoxExt, GtkWindowExt, WidgetExt},
};
use gtk4_layer_shell::*;
use hyprland::{data::Clients, shared::HyprData};
use hyprland_screen_picker_protocols::{frame::FrameManager, output::OutputManager};
use wayland_client::Connection;

use crate::{config::WindowConfig, image::Image};

const APP_ID: &str = "ch.wysbd.hyprland-screen-picker";
const IMAGE_TARGET_SIZE: i32 = 150;

pub struct App {
    gtk_app: Application,
}

impl App {
    pub fn build(inspector: bool, window_config: &WindowConfig) -> Self {
        if inspector {
            unsafe {
                std::env::set_var("GTK_DEBUG", "interactive");
            }
        }

        let gtk_app = Application::builder().application_id(APP_ID).build();

        let app = Self { gtk_app };

        app.gtk_app.connect_startup(move |_| {
            // TODO: load stylesheets
        });

        let window_config_c = window_config.clone();
        app.gtk_app.connect_activate(move |app| {
            let window = Self::build_window(app, &window_config_c);

            let con = Connection::connect_to_env().expect("should connect");

            let notebook = gtk::Notebook::builder().build();

            let windows_view = Self::build_windows_view(&con);
            let windows_label = gtk::Label::builder().label("Windows").build();
            let outputs_view = Self::build_outputs_view(&con);
            let outputs_label = gtk::Label::builder().label("Outputs").build();
            let region_view = Self::build_region_view();
            let region_label = gtk::Label::builder().label("Region").build();

            notebook.append_page(&windows_view, Some(&windows_label));
            notebook.append_page(&outputs_view, Some(&outputs_label));
            notebook.append_page(&region_view, Some(&region_label));

            window.set_child(Some(&notebook));
            window.present();
        });

        app
    }

    pub fn run(&self) -> ExitCode {
        let empty_args: Vec<String> = vec![];
        self.gtk_app.run_with_args(&empty_args)
    }

    fn build_window(app: &Application, config: &WindowConfig) -> ApplicationWindow {
        let window = ApplicationWindow::builder()
            .application(app)
            .destroy_with_parent(true)
            .default_width(config.width)
            .default_height(config.height)
            .build();

        window.init_layer_shell();
        window.set_layer(Layer::Top);
        window.set_exclusive_zone(-1);

        window
    }

    fn build_windows_view(con: &Connection) -> impl IsA<Widget> {
        let scrolled_container = ScrolledWindow::builder().build();
        let container =
            FlowBox::builder().vexpand(false).max_children_per_line(3).row_spacing(12).column_spacing(12).orientation(gtk4::Orientation::Horizontal).build();

        let mut manager = FrameManager::new(con).expect("should create frame manager");
        let clients = Clients::get().expect("should get hyprland clients");
        clients.iter().for_each(|client| {
            let handle = u64::from_str_radix(format!("{}", client.address)[2..].as_ref(), 16).expect("should be valid u64");
            let buffer = manager.capture_frame(handle).expect("should capture frame");
            let mut img = Image::new(buffer).expect("should create image").into_rgb().expect("should convert to rgb");

            img.resize_to_fit(200);
            let card = Self::build_image_with_label(img, client.title.as_str());
            container.insert(&card, 0);
        });

        scrolled_container.set_child(Some(&container));
        scrolled_container
    }
    fn build_outputs_view(con: &Connection) -> impl IsA<Widget> {
        let scrolled_container = ScrolledWindow::builder().build();
        let container =
            FlowBox::builder().hexpand(false).vexpand(false).row_spacing(12).column_spacing(12).orientation(gtk4::Orientation::Horizontal).build();

        let mut manager = OutputManager::new(con).expect("should create output manager");
        let outputs = manager.outputs.clone();
        outputs.into_iter().enumerate().for_each(|(index, (wl_output, output))| {
            let buffer = manager.capture_output(&wl_output).expect("should capture output");
            let mut img = Image::new(buffer).expect("should create image").into_rgb().expect("should convert to rgb");

            img.resize_to_fit(200);
            let label_text = output.name.unwrap_or(format!("Output {}", index + 1));
            let card = Self::build_image_with_label(img, label_text.as_str());
            container.insert(&card, 0);
        });

        scrolled_container.set_child(Some(&container));
        scrolled_container
    }

    fn build_region_view() -> impl IsA<Widget> {
        let container = Box::builder().orientation(gtk4::Orientation::Vertical).build();
        container
    }

    fn build_image_with_label(image: Image, label_text: &str) -> impl IsA<Widget> {
        let container = Box::builder().orientation(gtk4::Orientation::Vertical).spacing(0).build();

        let aspect_ratio = image.aspect_ratio;
        let pixbuf = image.into_pixbuf().expect("should be valid pixbuf");

        let image = GtkImage::from_pixbuf(Some(&pixbuf));
        drop(pixbuf);
        let label =
            Label::builder().max_width_chars(30).label(label_text).single_line_mode(true).vexpand(false).hexpand(false).build();

        // let (height, width) = if aspect_ratio > 1.0 {
        //     (IMAGE_TARGET_SIZE, (IMAGE_TARGET_SIZE as f64 / aspect_ratio) as i32)
        // } else {
        //     ((IMAGE_TARGET_SIZE as f64 / aspect_ratio) as i32, IMAGE_TARGET_SIZE)
        // };
        let (width, height) = ((IMAGE_TARGET_SIZE as f32 * 16_f32 / 9_f32) as i32, IMAGE_TARGET_SIZE);
        image.set_width_request(width);
        image.set_height_request(height);

        container.append(&image);
        container.append(&label);

        container
    }
}
