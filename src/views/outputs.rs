use std::sync::Arc;

use glib::{clone, variant::ToVariant};
use gtk4::{
    Box, Button, Fixed, GestureClick, Label, Picture, ScrolledWindow,
    prelude::{BoxExt, ButtonExt, EventControllerExt, FixedExt, WidgetExt, WidgetExtManual},
};
use hyprland::{
    data::{Monitor, Monitors},
    shared::HyprData,
};
use hyprland_preview_share_picker_lib::{image::Image, output::OutputManager};
use tokio::sync::oneshot::{Receiver, Sender};
use wayland_client::{Connection, protocol::wl_output::WlOutput};

use crate::{config::Config, image::ImageExt, util::MonitorTransformExt};

use super::View;

struct MonitorArea {
    aspect_ratio: f64,
    width: i32,
    height: i32,
    offset_x: i32,
    offset_y: i32,
}

impl From<&Vec<Monitor>> for MonitorArea {
    fn from(monitors: &Vec<Monitor>) -> Self {
        let min_x = monitors.iter().min_by_key(|m| m.x).map(|m| m.x).unwrap_or_default();
        let min_y = monitors.iter().min_by_key(|m| m.y).map(|m| m.y).unwrap_or_default();
        let max_x = monitors.iter().max_by_key(|m| m.x + m.width as i32).map(|m| m.x + m.width as i32).unwrap_or_default();
        let max_y = monitors.iter().max_by_key(|m| m.y + m.height as i32).map(|m| m.y + m.height as i32).unwrap_or_default();

        let width = max_x - min_x;
        let height = max_y - min_y;
        let offset_x = -min_x.min(0);
        let offset_y = -min_y.min(0);
        Self { width, height, aspect_ratio: width as f64 / height as f64, offset_x, offset_y }
    }
}

pub struct OutputsView<'a> {
    config: &'a Config,
    manager: Arc<OutputManager>,
    monitors: Vec<Monitor>,
    area: MonitorArea,
}

impl<'a> OutputsView<'a> {
    pub fn new(connection: &'a Connection, config: &'a Config) -> Result<Self, String> {
        let manager = OutputManager::new(connection)
            .map(Arc::new)
            .map_err(|err| format!("unable to create new output manager from connection: {err}"))?;
        let mut monitors = Monitors::get()
            .map(|monitors| monitors.into_iter().collect::<Vec<_>>())
            .map_err(|err| format!("unable to get monitors from hyprland socket: {err}"))?;

        // apply the transformations (rotations) to all monitors
        monitors.iter_mut().for_each(|m| m.apply_transform());
        let area = MonitorArea::from(&monitors);
        let mut view = Self { config, manager, monitors, area };
        if config.outputs.respect_output_scaling {
            view.apply_output_scaling();
        }
        Ok(view)
    }

    fn apply_output_scaling(&mut self) {
        // FIXME: The current solution does not work well for vertically stacked monitors
        self.monitors.sort_by(|a, b| a.x.cmp(&b.x));
        let mut translation = 0i32;
        self.monitors.iter_mut().for_each(|m| {
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
        self.monitors.sort_by(|a, b| a.y.cmp(&b.y));
        translation = 0;
        self.monitors.iter_mut().for_each(|m| {
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
}

impl View for OutputsView<'_> {
    fn build(&self) -> ScrolledWindow {
        let container = Fixed::builder().hexpand(false).vexpand(false).build();
        let scrolled_window =
            ScrolledWindow::builder().child(&container).css_classes([self.config.classes.notebook_page.as_str()]).build();

        self.manager.outputs.iter().for_each(|(wl_output, output)| {
            let name = match &output.name {
                Some(name) => name,
                None => return log::error!("output {output:?} does not have a name"),
            };
            let Some(monitor) = self.monitors.iter().find(|m| m.name.eq(name)).cloned() else {
                return log::error!("output {name} does not exist on hyprland");
            };
            let output_card = OutputCard::new(&monitor, self.config, wl_output, &self.area, self.manager.clone());
            let card = match output_card.build() {
                Ok(card) => card,
                Err(err) => return log::error!("unable to build output card for output {name}: {err}"),
            };
            output_card.append_on_allocation(&container, &card);
        });

        scrolled_window
    }

    fn label(&self) -> Label {
        Label::builder().css_classes([self.config.classes.tab_label.as_str()]).label("Outputs").build()
    }
}

struct OutputCard<'a> {
    monitor: &'a Monitor,
    config: &'a Config,
    manager: Arc<OutputManager>,
    output: &'a WlOutput,
    area: &'a MonitorArea,
}

impl<'a> OutputCard<'a> {
    fn new(
        monitor: &'a Monitor,
        config: &'a Config,
        output: &'a WlOutput,
        area: &'a MonitorArea,
        manager: Arc<OutputManager>,
    ) -> Self {
        Self { monitor, config, output, manager, area }
    }

    pub fn build(&self) -> Result<Button, String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let picture = self.build_picture();
        let card = self.build_card(&picture);
        let container = self.build_card_container(&card);

        self.request_frame(tx);
        self.update_frame_lazily(card.clone(), picture.clone(), rx);

        Ok(container)
    }

    fn build_picture(&self) -> Picture {
        Picture::builder()
            .vexpand(true)
            .valign(gtk4::Align::Fill)
            .halign(gtk4::Align::Fill)
            .content_fit(gtk4::ContentFit::Fill)
            .css_classes([self.config.classes.image.as_str()])
            .build()
    }

    fn build_card(&self, picture: &Picture) -> Box {
        let container = Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .vexpand(false)
            .hexpand(false)
            .halign(gtk4::Align::Fill)
            .valign(gtk4::Align::Fill)
            .margin_end(self.config.outputs.spacing as i32)
            .margin_start(self.config.outputs.spacing as i32)
            .margin_top(self.config.outputs.spacing as i32)
            .margin_bottom(self.config.outputs.spacing as i32)
            .css_classes([self.config.classes.image_card.as_str(), self.config.classes.image_card_loading.as_str()])
            .build();

        let label = Label::builder()
            .max_width_chars(1)
            .label(&self.monitor.name)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .single_line_mode(true)
            .css_classes([self.config.classes.image_label.as_str()])
            .hexpand(false)
            .build();

        container.append(picture);
        container.append(&label);
        container
    }

    fn build_card_container(&self, card: &Box) -> Button {
        let container = Button::builder().focusable(true).child(card).build();

        let gesture = GestureClick::new();
        gesture.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let clicks = self.config.windows.clicks;
        let name = &self.monitor.name;
        gesture.connect_released(clone!(
            #[strong]
            name,
            move |gesture, n, _, _| {
                if n as i64 == clicks as i64 {
                    if let Some(widget) = gesture.widget() {
                        widget
                            .activate_action("win.select", Some(&format!("screen:{name}").to_variant()))
                            .expect("select action should be registered on the window")
                    }
                }
            }
        ));
        container.add_controller(gesture);
        container.connect_activate(clone!(
            #[strong]
            name,
            move |child| {
                child
                    .activate_action("win.select", Some(&format!("screen:{name}").to_variant()))
                    .expect("select action should be registered on the window")
            }
        ));
        container
    }

    pub fn append_on_allocation(&self, container: &Fixed, card: &Button) {
        let &MonitorArea { aspect_ratio, width: monitors_width, height: monitors_height, offset_x, offset_y, .. } =
            self.area;
        let &Monitor { height, width, x, y, .. } = self.monitor;

        container.add_tick_callback(clone!(
            #[strong]
            card,
            move |container, _| {
                let allocation = container.allocation();
                // listen to ticks until we have an allocation
                if allocation.width() == 0 || allocation.height() == 0 {
                    glib::ControlFlow::Continue
                } else {
                    let container_aspect_ratio = allocation.width() as f64 / allocation.height() as f64;
                    let monitors_width_f = monitors_width as f64;
                    let monitors_height_f = monitors_height as f64;
                    let transform_x = |x: i32| {
                        if aspect_ratio > container_aspect_ratio {
                            (x as f64 / monitors_width_f) * allocation.width() as f64
                        } else {
                            (x as f64 / monitors_width_f) * allocation.height() as f64 * aspect_ratio
                        }
                    };
                    let transform_y = |y: i32| {
                        if aspect_ratio > container_aspect_ratio {
                            (y as f64 / monitors_height_f) * allocation.width() as f64 / aspect_ratio
                        } else {
                            (y as f64 / monitors_height_f) * allocation.height() as f64
                        }
                    };

                    card.set_width_request(transform_x(width as i32) as i32);
                    card.set_height_request(transform_y(height as i32) as i32);

                    let transformed_monitor_width = transform_x(monitors_width);
                    let transformed_monitor_height = transform_x(monitors_height);

                    let px_offset_x = (allocation.width() as f64 - transformed_monitor_width).max(0.0) / 2.0;
                    let px_offset_y = (allocation.height() as f64 - transformed_monitor_height).max(0.0) / 2.0;

                    container.put(&card, px_offset_x + transform_x(offset_x + x), px_offset_y + transform_y(offset_y + y));
                    glib::ControlFlow::Break
                }
            }
        ));
    }

    fn request_frame(&self, tx: Sender<Image>) {
        let resize_size = self.config.image.resize_size;
        let manager = self.manager.clone();
        let name = &self.monitor.name;
        let output = self.output;
        let transform = self.monitor.transform;

        tokio::spawn(clone!(
            #[strong]
            name,
            #[strong]
            output,
            #[to_owned]
            manager,
            async move {
                let buffer = match manager.to_owned().capture_output(&output) {
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
                img = img.transform(transform.into());

                if tx.send(img).is_err() {
                    log::error!("unable to transmit image for name {name}: channel is closed");
                };
                log::debug!("transmitted image for output {name}");
            }
        ));
    }

    fn update_frame_lazily(&self, card: Box, picture: Picture, rx: Receiver<Image>) {
        let loading_class = self.config.classes.image_card_loading.clone();
        let name = self.monitor.name.clone();
        glib::spawn_future_local(async move {
            let img = match rx.await {
                Ok(img) => img,
                Err(err) => {
                    log::error!("unable to receive image for output {name}: {err}");
                    card.remove_css_class(&loading_class);
                    return;
                }
            };

            let pixbuf = match img.into_pixbuf() {
                Ok(pixbuf) => pixbuf,
                Err(err) => return log::error!("unable to create pixbuf for output {name} image: {err}"),
            };

            picture.set_pixbuf(Some(&pixbuf));
            card.remove_css_class(&loading_class);
        });
    }
}
