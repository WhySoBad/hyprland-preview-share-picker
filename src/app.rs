use std::{
    cell::RefCell,
    process::{Command, exit},
    rc::Rc,
};

use glib::variant::{StaticVariantType, ToVariant};
use gtk4::{
    Application, ApplicationWindow, Box, Button, CheckButton, CssProvider, EventControllerKey, Fixed, FlowBox, FlowBoxChild,
    GestureClick, Label, Notebook, Picture, STYLE_PROVIDER_PRIORITY_APPLICATION, ScrolledWindow, Widget,
    gdk::Display,
    gio::{
        ActionEntry,
        prelude::{ActionMapExtManual, ApplicationExt, ApplicationExtManual},
    },
    glib::{ExitCode, clone, object::IsA},
    prelude::{
        BoxExt, ButtonExt, CheckButtonExt, EventControllerExt, FixedExt, FlowBoxChildExt, GtkWindowExt, WidgetExt,
        WidgetExtManual,
    },
};
use gtk4_layer_shell::*;
use hyprland::{
    data::{Clients, Monitors},
    shared::HyprData,
};
use hyprland_preview_share_picker_lib::{frame::FrameManager, image::Image, output::OutputManager, toplevel::Toplevel};
use regex::Regex;
use rsass::{compile_scss, output};
use wayland_client::Connection;

use crate::{config::Config, image::ImageExt, util::MonitorTransformExt};

const APP_ID: &str = "ch.wysbd.hyprland-preview-screen-picker";

pub struct App {
    gtk_app: Application,
}

impl App {
    pub fn build(interactive_debug: bool, config: Config, toplevels: Vec<Toplevel>, restore_token: bool) -> Self {
        let gtk_app = Application::builder().application_id(APP_ID).build();

        let app = Self { gtk_app };

        app.gtk_app.connect_startup(clone!(
            #[strong]
            config,
            move |_| {
                load_stylesheets(&config);
            }
        ));

        if interactive_debug {
            if let Err(err) = gtk4::glib::setenv("GTK_DEBUG", "interactive", true) {
                log::error!("unable to open gtk interactive debugger: {err}")
            } else {
                log::info!("opened interactive debugger")
            }
        }

        app.gtk_app.connect_activate(move |app| {
            log::debug!("gtk app is activated");
            build_ui(app, &config, &toplevels, restore_token);
        });

        app
    }

    pub fn run(&self) -> ExitCode {
        let empty_args: Vec<String> = vec![];
        self.gtk_app.run_with_args(&empty_args)
    }
}

fn build_ui(app: &Application, config: &Config, toplevels: &[Toplevel], default_restore_token: bool) {
    let window = build_window(app, config);
    log::debug!("built application window");
    let window_container = Box::new(gtk4::Orientation::Vertical, 0);
    window.set_child(Some(&window_container));

    let con = match Connection::connect_to_env() {
        Ok(connection) => connection,
        Err(err) => {
            log::error!("unable to connect to wayland server: {err}");
            exit(1);
        }
    };

    let restore_token = Rc::new(RefCell::new(default_restore_token));
    let exit_action = ActionEntry::builder("select")
        .parameter_type(Some(&String::static_variant_type()))
        .activate(clone!(
            #[strong]
            restore_token,
            move |_: &ApplicationWindow, _, parameter| {
                let allow_restore_token = *restore_token.borrow();
                let parameter = parameter
                    .expect("win.select called without parameter")
                    .get::<String>()
                    .expect("parameter of win.select action should be a string");
                println!("[SELECTION]{}/{parameter}", if allow_restore_token { "r" } else { "" });
                exit(0);
            }
        ))
        .build();
    window.add_action_entries([exit_action]);

    let notebook = Notebook::builder().css_classes([config.classes.notebook.as_str()]).vexpand(true).build();

    let windows_view = build_windows_view(&con, toplevels, config);
    let windows_label = Label::builder().css_classes([config.classes.tab_label.as_str()]).label("Windows").build();
    let outputs_view = build_outputs_view(&con, config);
    let outputs_label = Label::builder().css_classes([config.classes.tab_label.as_str()]).label("Outputs").build();
    let region_view = build_region_view(config);
    let region_label = Label::builder().css_classes([config.classes.tab_label.as_str()]).label("Region").build();

    let windows_page_num = notebook.append_page(&windows_view, Some(&windows_label));
    let outputs_page_num = notebook.append_page(&outputs_view, Some(&outputs_label));
    let region_page_num = notebook.append_page(&region_view, Some(&region_label));

    notebook.set_current_page(Some(match config.default_page {
        crate::config::Page::Windows => windows_page_num,
        crate::config::Page::Outputs => outputs_page_num,
        crate::config::Page::Region => region_page_num,
    }));

    window_container.append(&notebook);

    if !config.hide_token_restore {
        log::debug!("building token restore widget");
        let restore_button = build_restore_checkbox(restore_token, config);
        window_container.append(&restore_button);
    }

    log::debug!("presenting window");
    window.present();
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
        .vexpand(false)
        .hexpand(false)
        .css_classes([config.classes.window.as_str()])
        .build();

    let event_controller = EventControllerKey::new();
    event_controller.connect_key_pressed(|_, key, _, _| {
        if let gtk4::gdk::Key::Escape = key {
            log::debug!("exiting: escape key pressed");
            exit(0);
        }
        gtk4::glib::Propagation::Proceed
    });
    window.add_controller(event_controller);

    window.init_layer_shell();
    window.set_namespace(APP_ID);
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::OnDemand);
    window.set_exclusive_zone(-1);

    window
}

fn build_windows_view(con: &Connection, toplevels: &[Toplevel], config: &Config) -> impl IsA<Widget> {
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

    let manager = match FrameManager::new(con) {
        Ok(manager) => std::sync::Arc::new(manager),
        Err(err) => {
            log::error!("unable to create new frame manager from connection: {err}");
            return scrolled_container;
        }
    };
    let clients = match Clients::get() {
        Ok(clients) => Vec::from_iter(clients),
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

        let resize_size = config.image.resize_size;
        let id = toplevel.id;
        let (card, image) = build_window_image_with_label(toplevel.title.as_str(), config);
        let (tx, rx) = tokio::sync::oneshot::channel();

        tokio::spawn(clone!(
            #[to_owned]
            manager,
            async move {
                let mut manager = manager;
                let buffer = match manager.capture_frame(handle) {
                    Ok(buffer) => buffer,
                    Err(err) => return log::error!("unable to capture frame for toplevel {id}: {err}"),
                };
                let mut img = match Image::new(buffer) {
                    Ok(img) => match img.into_rgb() {
                        Ok(img) => img,
                        Err(err) => return log::error!("unable to convert Xrgb image to rgb: {err}"),
                    },
                    Err(err) => return log::error!("unable to create image from buffer: {err}"),
                };

                img.resize_to_fit(resize_size);

                if tx.send(img).is_err() {
                    log::error!("unable to transmit image for toplevel {id}: channel is closed");
                };
                log::debug!("transmitted image for toplevel {id}");
            }
        ));

        let card_loading_css = config.classes.image_card_loading.clone();
        glib::spawn_future_local(clone!(
            #[strong]
            card,
            async move {
                let img = match rx.await {
                    Ok(img) => img,
                    Err(err) => {
                        log::error!("unable to receive image for toplevel {id}: {err}");
                        card.remove_css_class(card_loading_css.as_str());
                        return;
                    }
                };
                log::debug!("received image for toplevel {id}");

                let pixbuf = match img.into_pixbuf() {
                    Ok(pixbuf) => pixbuf,
                    Err(err) => return log::error!("unable to create pixbuf for toplevel {id} image: {err}"),
                };
                image.set_pixbuf(Some(&pixbuf));
                card.remove_css_class(card_loading_css.as_str());
            }
        ));

        let flowbox_child = FlowBoxChild::builder().halign(gtk4::Align::Fill).valign(gtk4::Align::Fill).child(&card).build();

        let gesture = GestureClick::new();
        let clicks = config.windows.clicks;
        gesture.connect_released(move |gesture, n, _, _| {
            if n < 0 || (n as u32) < clicks {
                return;
            }
            if let Some(widget) = gesture.widget() {
                widget
                    .activate_action("win.select", Some(&format!("window:{id}").to_variant()))
                    .expect("select action should be registered on the window")
            }
        });
        flowbox_child.add_controller(gesture);
        flowbox_child.connect_activate(move |child| {
            child
                .activate_action("win.select", Some(&format!("window:{id}").to_variant()))
                .expect("select action should be registered on the window")
        });

        container.insert(&flowbox_child, 0);
    });

    scrolled_container
}

fn build_outputs_view(con: &Connection, config: &Config) -> impl IsA<Widget> {
    let scrolled_container = ScrolledWindow::builder().css_classes([config.classes.notebook_page.as_str()]).build();
    let container = Fixed::builder().hexpand(false).vexpand(false).build();
    scrolled_container.set_child(Some(&container));

    let manager = match OutputManager::new(con) {
        Ok(manager) => std::sync::Arc::new(manager),
        Err(err) => {
            log::error!("unable to create new output manager from connection: {err}");
            return scrolled_container;
        }
    };
    let mut monitors = match Monitors::get() {
        Ok(monitors) => Vec::from_iter(monitors),
        Err(err) => {
            log::error!("unable to get monitors form hyprland socket: {err}");
            Vec::new()
        }
    };
    let outputs = manager.outputs.clone();

    monitors.iter_mut().for_each(|m| m.apply_transform());
    if config.outputs.respect_output_scaling {
        monitors.sort_by(|a, b| a.x.cmp(&b.x));
        let mut translation = 0i32;
        monitors.iter_mut().for_each(|m| {
            m.x += translation;
            if m.scale != 1.0 {
                let new_width = (m.width as f32 / m.scale) as u16;
                if new_width > m.width {
                    translation += (new_width - m.width) as i32;
                } else {
                    translation -= (m.width - new_width) as i32;
                }
                m.width = new_width;
            }
        });
        monitors.sort_by(|a, b| a.y.cmp(&b.y));
        translation = 0;
        monitors.iter_mut().for_each(|m| {
            m.y += translation;
            if m.scale != 1.0 {
                let new_height = (m.height as f32 / m.scale) as u16;
                if new_height > m.height {
                    translation += (new_height - m.height) as i32;
                } else {
                    translation -= (m.height - new_height) as i32;
                }
                m.height = new_height;
            }
        });
    }
    let min_x = monitors.iter().min_by_key(|m| m.x).map(|m| m.x as f64).unwrap_or_default();
    let min_y = monitors.iter().min_by_key(|m| m.y).map(|m| m.y as f64).unwrap_or_default();
    let max_x =
        monitors.iter().max_by_key(|m| m.x + m.width as i32).map(|m| (m.x + m.width as i32) as f64).unwrap_or_default();
    let max_y =
        monitors.iter().max_by_key(|m| m.y + m.height as i32).map(|m| (m.y + m.height as i32) as f64).unwrap_or_default();

    let monitors_width = max_x - min_x;
    let monitors_height = max_y - min_y;
    let monitors_aspect_ratio = monitors_width / monitors_height;
    let offset_x = -min_x.min(0.0);
    let offset_y = -min_y.min(0.0);

    outputs.into_iter().for_each(|(wl_output, output)| {
        let name = match output.name {
            Some(name) => name,
            None => return log::error!("output {output:?} does not have a name"),
        };

        let monitor = monitors.iter().find(|m| m.name.eq(&name)).cloned();
        let (x, y, width, height) =
            monitor.as_ref().map(|m| (m.x as f64, m.y as f64, m.width as f64, m.height as f64)).unwrap_or_default();

        let resize_size = config.image.resize_size;
        let (card, image) = build_output_image_with_label(name.as_str(), config);
        let (tx, rx) = tokio::sync::oneshot::channel();

        tokio::spawn(clone!(
            #[strong]
            name,
            #[to_owned]
            manager,
            async move {
                let mut manager = manager;
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

                img.resize_to_fit(resize_size);
                if let Some(monitor) = monitor {
                    img = img.transform(monitor.transform.into());
                }

                if tx.send(img).is_err() {
                    log::error!("unable to transmit image for name {name}: channel is closed");
                };
                log::debug!("transmitted image for output {name}");
            }
        ));

        let card_loading_css = config.classes.image_card_loading.clone();
        glib::spawn_future_local(clone!(
            #[strong]
            name,
            #[strong]
            card,
            async move {
                let img = match rx.await {
                    Ok(img) => img,
                    Err(err) => {
                        log::error!("unable to receive image for output {name}: {err}");
                        card.remove_css_class(card_loading_css.as_str());
                        return;
                    }
                };
                log::debug!("received image for output {name}");

                let pixbuf = match img.into_pixbuf() {
                    Ok(pixbuf) => pixbuf,
                    Err(err) => return log::error!("unable to create pixbuf for output {name} image: {err}"),
                };
                image.set_pixbuf(Some(&pixbuf));
                card.remove_css_class(card_loading_css.as_str());
            }
        ));

        let button = Button::builder().focusable(true).child(&card).build();

        let gesture = GestureClick::new();
        gesture.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let clicks = config.outputs.clicks;
        gesture.connect_released(clone!(
            #[strong]
            name,
            move |gesture, n, _, _| {
                if n < 0 || (n as u32) < clicks {
                    return;
                }
                if let Some(widget) = gesture.widget() {
                    widget
                        .activate_action("win.select", Some(&format!("screen:{name}").to_variant()))
                        .expect("select action should be registered on the window")
                }
            }
        ));
        button.add_controller(gesture);
        button.connect_activate(move |child| {
            child
                .activate_action("win.select", Some(&format!("screen:{name}").to_variant()))
                .expect("select action should be registered on the window")
        });

        container.add_tick_callback(clone!(
            #[strong]
            button,
            move |widget, _| {
                let allocation = widget.allocation();
                // listen to ticks until we have an allocation
                if allocation.width() == 0 || allocation.height() == 0 {
                    glib::ControlFlow::Continue
                } else {
                    let aspect_ratio = allocation.width() as f64 / allocation.height() as f64;
                    let transform_x = |x: f64| {
                        if monitors_aspect_ratio > aspect_ratio {
                            (x / monitors_width) * allocation.width() as f64
                        } else {
                            (x / monitors_width) * allocation.height() as f64 * monitors_aspect_ratio
                        }
                    };
                    let transform_y = |y: f64| {
                        if monitors_aspect_ratio > aspect_ratio {
                            (y / monitors_height) * allocation.width() as f64 / monitors_aspect_ratio
                        } else {
                            (y / monitors_height) * allocation.height() as f64
                        }
                    };

                    log::debug!("height = {height}, monitors_height = {monitors_height}");

                    button.set_width_request(transform_x(width) as i32);
                    button.set_height_request(transform_y(height) as i32);

                    log::debug!("width_request = {}, height_request = {}", transform_x(width), transform_y(height));

                    let transformed_monitor_width = transform_x(monitors_width);
                    let transformed_monitor_height = transform_x(monitors_height);

                    let px_offset_x = (allocation.width() as f64 - transformed_monitor_width).max(0.0) / 2.0;
                    let px_offset_y = (allocation.height() as f64 - transformed_monitor_height).max(0.0) / 2.0;

                    log::debug!("px_offset_x = {px_offset_x}, px_offset_y = {px_offset_y}");
                    log::debug!(
                        "transform_x = {}, transform_y = {}, x = {}, y = {}",
                        transform_x(width),
                        transform_y(height),
                        transform_x(offset_x + x),
                        transform_y(offset_y + y)
                    );

                    widget.put(&button, px_offset_x + transform_x(offset_x + x), px_offset_y + transform_y(offset_y + y));
                    glib::ControlFlow::Break
                }
            }
        ));
    });

    scrolled_container
}

fn build_region_view(config: &Config) -> impl IsA<Widget> {
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

    button.connect_clicked(move |btn| {
        if let Some(root) = btn.root() {
            if let Some(args) = &args {
                let mut command = Command::new(&args[0]);
                command.args(&args[1..]);
                log::info!("using {command:?} as region command");
                root.hide();

                let region_regex = region_regex.clone();
                glib::spawn_future_local(async move {
                    match command.output() {
                        Ok(output) => {
                            let region = String::from_utf8_lossy(&output.stdout);
                            let region = region.trim();
                            if region_regex.is_match(region) {
                                root.activate_action("win.select", Some(&format!("region:{region}").to_variant()))
                                    .expect("select action should be registered on the window");
                            } else {
                                log::error!(
                                    "region command returned output '{region}': expected '<output>@<x>,<y>,<w>,<h>'"
                                );
                                root.show();
                            }
                        }
                        Err(err) => {
                            log::error!("error whilst selecting share region: {err}");
                            root.show();
                        }
                    }
                });
            }
        }
    });

    container.insert_child_after(&button, Option::<&Box>::None);

    container
}

fn build_restore_checkbox(restore_token: Rc<RefCell<bool>>, config: &Config) -> impl IsA<Widget> {
    let button = CheckButton::builder()
        .css_classes([config.classes.restore_button.as_str()])
        .label("Allow a restore token")
        .active(*restore_token.borrow())
        .build();

    button.connect_toggled(move |btn| {
        *restore_token.borrow_mut() = btn.is_active();
    });

    button
}

fn build_window_image_with_label(label_text: &str, config: &Config) -> (impl IsA<Widget>, Picture) {
    let container = Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .vexpand(false)
        .hexpand(false)
        .halign(gtk4::Align::Center)
        .valign(gtk4::Align::Start)
        .css_classes([config.classes.image_card.as_str(), config.classes.image_card_loading.as_str()])
        .build();

    let image = Picture::builder()
        .vexpand(true)
        .valign(gtk4::Align::Center)
        // .height_request(config.image.widget_size)
        .content_fit(gtk4::ContentFit::Contain)
        .css_classes([config.classes.image.as_str()])
        .build();

    container.append(&image);

    let label = Label::builder()
        .max_width_chars(1)
        .label(label_text)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .single_line_mode(true)
        .css_classes([config.classes.image_label.as_str()])
        .hexpand(false)
        .build();

    container.append(&label);

    (container, image)
}

fn build_output_image_with_label(label_text: &str, config: &Config) -> (impl IsA<Widget>, Picture) {
    let container = Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .vexpand(false)
        .hexpand(false)
        .halign(gtk4::Align::Fill)
        .valign(gtk4::Align::Fill)
        .margin_end(config.outputs.spacing as i32)
        .margin_start(config.outputs.spacing as i32)
        .margin_top(config.outputs.spacing as i32)
        .margin_bottom(config.outputs.spacing as i32)
        .css_classes([config.classes.image_card.as_str(), config.classes.image_card_loading.as_str()])
        .build();

    let image = Picture::builder()
        .vexpand(true)
        .valign(gtk4::Align::Fill)
        .halign(gtk4::Align::Fill)
        .content_fit(gtk4::ContentFit::Fill)
        .css_classes([config.classes.image.as_str()])
        .build();

    container.append(&image);

    if config.outputs.show_label {
        let label = Label::builder()
            .max_width_chars(1)
            .label(label_text)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .single_line_mode(true)
            .css_classes([config.classes.image_label.as_str()])
            .hexpand(false)
            .build();

        container.append(&label);
    }

    (container, image)
}
