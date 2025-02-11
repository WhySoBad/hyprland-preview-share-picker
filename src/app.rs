use std::process::{Command, exit};

use gtk4::{
    Application, ApplicationWindow, Box, Button, CssProvider, EventControllerKey, FlowBox, FlowBoxChild, GestureClick,
    Label, Notebook, Picture, STYLE_PROVIDER_PRIORITY_APPLICATION, ScrolledWindow, Widget,
    gdk::{Display, Texture},
    gio::prelude::{ApplicationExt, ApplicationExtManual},
    glib::{ExitCode, object::IsA, spawn_future_local},
    prelude::{BoxExt, ButtonExt, FlowBoxChildExt, GtkWindowExt, WidgetExt},
};
use gtk4_layer_shell::*;
use hyprland::{
    data::{Clients, Monitors},
    shared::HyprData,
};
use hyprland_share_picker_protocols::{frame::FrameManager, output::OutputManager};
use regex::Regex;
use rsass::{compile_scss, output};
use wayland_client::Connection;

use crate::{config::Config, image::Image, toplevel::Toplevel};

const APP_ID: &str = "ch.wysbd.hyprland-screen-picker";

pub struct App {
    gtk_app: Application,
}

impl App {
    pub fn build(inspector: bool, config: Config, toplevels: Vec<Toplevel>) -> Self {
        if inspector {
            unsafe {
                std::env::set_var("GTK_DEBUG", "interactive");
            }
        }

        let gtk_app = Application::builder().application_id(APP_ID).build();

        let app = Self { gtk_app };

        let config_clone = config.clone();
        app.gtk_app.connect_startup(move |_| {
            Self::load_stylesheets(&config_clone);
        });

        app.gtk_app.connect_activate(move |app| {
            let window = Self::build_window(app, &config);

            let con = match Connection::connect_to_env() {
                Ok(connection) => connection,
                Err(err) => {
                    log::error!("unable to connect to wayland server: {err}");
                    exit(1);
                }
            };

            let notebook = Notebook::builder().css_classes([config.classes.notebook.as_str()]).build();

            let windows_view = Self::build_windows_view(&con, &toplevels, &config);
            let windows_label = Label::builder().css_classes([config.classes.tab_label.as_str()]).label("Windows").build();
            let outputs_view = Self::build_outputs_view(&con, &config);
            let outputs_label = Label::builder().css_classes([config.classes.tab_label.as_str()]).label("Outputs").build();
            let region_view = Self::build_region_view(&window, &config);
            let region_label = Label::builder().css_classes([config.classes.tab_label.as_str()]).label("Region").build();

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

    fn load_stylesheets(config: &Config) {
        let provider = CssProvider::new();
        let format = output::Format { style: output::Style::Expanded, ..Default::default() };

        config.stylesheets.iter().for_each(|path_str| {
            let path = &config.resolve_path(path_str);
            if path.exists() {
                match std::fs::read(path) {
                    Ok(content) => {
                        let css = if path.extension().is_some_and(|ext| ext == "scss") {
                            match compile_scss(content.as_slice(), format) {
                                Ok(css) => css,
                                Err(err) => {
                                    log::error!("unable to compile stylesheet {path_str}: {err}");
                                    Vec::new()
                                }
                            }
                        } else {
                            content
                        };
                        let str = std::str::from_utf8(css.as_slice()).expect("should be valid utf-8");
                        provider.load_from_data(str);
                    }
                    Err(err) => log::error!("unable to read stylesheet from {path_str}: {err}"),
                }
            } else {
                log::warn!("style path {path_str} does not exist");
            }
        });

        gtk4::style_context_add_provider_for_display(
            &Display::default().expect("should have display"),
            &provider,
            STYLE_PROVIDER_PRIORITY_APPLICATION,
        )
    }

    fn build_window(app: &Application, config: &Config) -> ApplicationWindow {
        let window = ApplicationWindow::builder()
            .application(app)
            .destroy_with_parent(true)
            .default_width(config.window.width)
            .default_height(config.window.height)
            .css_classes([config.classes.window.as_str()])
            .build();

        let event_controller = EventControllerKey::new();
        event_controller.connect_key_pressed(|_, key, _, _| {
            match key {
                gtk4::gdk::Key::Escape => {
                    exit(0);
                }
                _ => (),
            }
            gtk4::glib::Propagation::Proceed
        });
        window.add_controller(event_controller);

        window.init_layer_shell();
        window.set_layer(Layer::Top);
        window.set_keyboard_mode(KeyboardMode::OnDemand);
        window.set_exclusive_zone(-1);

        window
    }

    fn build_windows_view(con: &Connection, toplevels: &Vec<Toplevel>, config: &Config) -> impl IsA<Widget> {
        let scrolled_container = ScrolledWindow::builder().css_classes([config.classes.notebook_page.as_str()]).build();
        let container = FlowBox::builder()
            .vexpand(false)
            .homogeneous(false)
            .row_spacing(12)
            .column_spacing(12)
            .orientation(gtk4::Orientation::Horizontal)
            .homogeneous(true)
            .min_children_per_line(config.windows.min_per_row)
            .max_children_per_line(config.windows.max_per_row)
            .build();
        scrolled_container.set_child(Some(&container));

        let mut manager = match FrameManager::new(con) {
            Ok(manager) => manager,
            Err(err) => {
                log::error!("unable to create new frame manager from connection: {err}");
                return scrolled_container;
            }
        };
        let clients = match Clients::get() {
            Ok(clients) => Vec::from_iter(clients.into_iter()),
            Err(err) => {
                log::error!("unable to get clients form hyprland socket: {err}");
                Vec::new()
            }
        };
        toplevels.iter().for_each(|toplevel| {
            log::debug!("attempting to capture frame for toplevel {}", toplevel.id);
            // this method is kindof bad since multiple windows could have the same class and title but afaik there is no clean
            // way to get a hyprland window address for a wayland toplevel id
            log::debug!("toplevel = {toplevel:?}");
            let client = match clients.iter().find(|c| c.class.eq(&toplevel.class) && c.title.eq(&toplevel.title)) {
                Some(client) => client,
                None => return log::error!("unable to find hyprland client which matches toplevel class and title"),
            };

            let handle = u64::from_str_radix(format!("{}", client.address)[2..].as_ref(), 16).expect("should be valid u64");
            let buffer = match manager.capture_frame(handle) {
                Ok(buf) => buf,
                Err(err) => return log::error!("unable to capture frame for toplevel {}: {}", toplevel.id, err),
            };
            let mut img = match Image::new(buffer) {
                Ok(img) => match img.into_rgb() {
                    Ok(img) => img,
                    Err(err) => return log::error!("unable to convert Xrgb image to rgb: {err}"),
                },
                Err(err) => return log::error!("unable to create image from buffer: {err}"),
            };

            img.resize_to_fit_height(config.image.resize_size);
            let card = match Self::build_image_with_label(img, toplevel.title.as_str(), config) {
                Ok(card) => card,
                Err(err) => return log::error!("unable to create image with label for toplevel {}: {err}", toplevel.id),
            };
            let flowbox_child =
                FlowBoxChild::builder().halign(gtk4::Align::Fill).valign(gtk4::Align::Fill).child(&card).build();

            let id = toplevel.id;
            let gesture = GestureClick::new();
            gesture.connect_released(move |_, n, _, _| {
                if n != 2 {
                    return;
                }
                Self::print_and_exit("window", &id.to_string());
            });
            flowbox_child.add_controller(gesture);
            flowbox_child.connect_activate(move |_| {
                Self::print_and_exit("window", &id.to_string());
            });

            container.insert(&flowbox_child, 0);
        });

        scrolled_container
    }

    fn build_outputs_view(con: &Connection, config: &Config) -> impl IsA<Widget> {
        let scrolled_container = ScrolledWindow::builder().css_classes([config.classes.notebook_page.as_str()]).build();
        let container = FlowBox::builder()
            .hexpand(false)
            .vexpand(false)
            .row_spacing(12)
            .column_spacing(12)
            .selection_mode(gtk4::SelectionMode::Browse)
            .orientation(gtk4::Orientation::Horizontal)
            .homogeneous(true)
            .min_children_per_line(config.outputs.min_per_row)
            .max_children_per_line(config.outputs.max_per_row)
            .build();

        scrolled_container.set_child(Some(&container));

        let mut manager = match OutputManager::new(con) {
            Ok(manager) => manager,
            Err(err) => {
                log::error!("unable to create new output manager from connection: {err}");
                return scrolled_container;
            }
        };
        let monitors = match Monitors::get() {
            Ok(monitors) => Vec::from_iter(monitors.into_iter()),
            Err(err) => {
                log::error!("unable to get monitors form hyprland socket: {err}");
                Vec::new()
            }
        };
        let outputs = manager.outputs.clone();

        outputs.into_iter().for_each(|(wl_output, output)| {
            let name = match output.name {
                Some(name) => name,
                None => return log::error!("output {output:?} does not have a name"),
            };
            let buffer = match manager.capture_output(&wl_output) {
                Ok(buffer) => buffer,
                Err(err) => return log::error!("unable to capture output {name}: {err}"),
            };
            let mut img = match Image::new(buffer) {
                Ok(img) => match img.into_rgb() {
                    Ok(img) => img,
                    Err(err) => return log::error!("unable to convert Xrgb image to rgb: {err}"),
                },
                Err(err) => return log::error!("unable to create image from buffer: {err}"),
            };

            if let Some(monitor) = monitors.iter().find(|m| m.name.eq(&name)) {
                img = img.transform(monitor.transform);
            }

            img.resize_to_fit_height(config.image.resize_size);
            let card = match Self::build_image_with_label(img, name.as_str(), config) {
                Ok(card) => card,
                Err(err) => return log::error!("unable to create image with label for output {name}: {err}"),
            };
            let flowbox_child =
                FlowBoxChild::builder().halign(gtk4::Align::Fill).valign(gtk4::Align::Fill).child(&card).build();

            let name_copy = name.clone();
            let gesture = GestureClick::new();
            gesture.connect_released(move |_, n, _, _| {
                if n != 2 {
                    return;
                }
                Self::print_and_exit("screen", name.as_str());
            });
            flowbox_child.add_controller(gesture);
            flowbox_child.connect_activate(move |_| {
                Self::print_and_exit("screen", name_copy.as_str());
            });

            container.insert(&flowbox_child, 0);
        });

        scrolled_container
    }

    fn build_region_view(window: &ApplicationWindow, config: &Config) -> impl IsA<Widget> {
        let container = Box::builder()
            .css_classes([config.classes.notebook_page.as_str()])
            .orientation(gtk4::Orientation::Vertical)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .build();

        let button = Button::builder().label("Select region").css_classes([config.classes.region_button.as_str()]).build();

        let args = if let Some(argv) = shlex::split(&config.region.command) {
            Some(argv)
        } else {
            log::error!("received invalid region command: {}", config.region.command);
            // disable the button
            button.set_sensitive(false);
            None
        };

        let region_regex = Regex::new(r"^.+@-?\d+,-?\d+,\d+,\d+$").expect("should be valid regex");

        let window = window.clone();
        button.connect_clicked(move |_| {
            if let Some(args) = &args {
                let mut command = Command::new(&args[0]);
                command.args(&args[1..]);
                log::info!("using {command:?} as region command");
                window.hide();

                let region_regex = region_regex.clone();
                let window = window.clone();
                spawn_future_local(async move {
                    match command.output() {
                        Ok(output) => {
                            let region = String::from_utf8_lossy(&output.stdout);
                            let region = region.trim();
                            if region_regex.is_match(&region) {
                                Self::print_and_exit("region", &region);
                            } else {
                                log::error!(
                                    "region command returned output '{region}': expected '<output>@<x>,<y>,<w>,<h>'"
                                );
                                window.show();
                            }
                        }
                        Err(err) => {
                            log::error!("error whilst selecting share region: {err}");
                            window.show();
                        }
                    }
                });
            }
        });

        container.insert_child_after(&button, Option::<&Box>::None);

        container
    }

    fn build_image_with_label(
        image: Image,
        label_text: &str,
        config: &Config,
    ) -> Result<impl IsA<Widget>, std::boxed::Box<dyn std::error::Error>> {
        let container = Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .vexpand(false)
            .hexpand(false)
            .halign(gtk4::Align::Fill)
            .valign(gtk4::Align::Fill)
            .css_classes([config.classes.image_card.as_str()])
            .build();
        let pixbuf = image.into_pixbuf()?;

        let texture = Texture::for_pixbuf(&pixbuf);
        let image = Picture::for_paintable(&texture);
        image.set_vexpand(true);
        image.set_valign(gtk4::Align::Center);
        drop(texture);
        drop(pixbuf);

        let label = Label::builder()
            .max_width_chars(1)
            .label(label_text)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .single_line_mode(true)
            .css_classes([config.classes.image_label.as_str()])
            .hexpand(false)
            .build();

        image.set_css_classes(&[config.classes.image.as_str()]);
        image.set_height_request(config.image.widget_size);
        image.set_content_fit(gtk4::ContentFit::Contain);

        container.append(&image);
        container.append(&label);

        Ok(container)
    }

    fn print_and_exit(reason: &str, argument: &str) {
        println!("[SELECTION]/{reason}:{argument}");
        exit(0)
    }
}
