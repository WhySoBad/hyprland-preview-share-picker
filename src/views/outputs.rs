use std::collections::HashMap;
use std::sync::Arc;

use glib::clone;
use glib::variant::ToVariant;
use gtk4::prelude::{BoxExt, ButtonExt, EventControllerExt, FixedExt, WidgetExt, WidgetExtManual};
use gtk4::{Box, Button, Fixed, GestureClick, Label, Picture, ScrolledWindow};
use hyprland_preview_share_picker_lib::image::Image;
use hyprland_preview_share_picker_lib::output::{Output, OutputManager};
use tokio::sync::oneshot::{Receiver, Sender};
use wayland_client::Connection;

use super::View;
use crate::config::Config;
use crate::image::ImageExt;
use crate::views::outputs::area::OutputArea;

/// A rectangular geometry consisting of (x, y, width, height).
type Geometry = (i32, i32, i32, i32);

mod area {
    use std::collections::hash_map::Values;

    use super::*;

    /// The area spanned by all outputs after having their rotations and scaling
    /// applied.
    pub struct OutputArea(HashMap<String, Geometry>, Geometry);

    impl OutputArea {
        /// Create a new [`OutputArea`] and apply the scaling of the [`Output`]s
        /// to their geometry.
        pub fn new_with_scaling(outputs: &[Output]) -> Self {
            let mut transformed = Self::new_without_scaling(outputs).0;

            // First, sort all outputs by ascending x-coordinate and apply the
            // transformations on the x-axis to them. If multiple outputs overlap,
            // the one which is further on the right is moved further to the right
            // such that the outputs no longer overlap.
            let mut sorted_by_x = outputs.iter().collect::<Vec<_>>();
            sorted_by_x.sort_by_key(|o| o.x);

            let mut translations_x = HashMap::<String, i32>::new();
            for output in &sorted_by_x {
                if output.is_scaled() {
                    let (_, _, width, height) = transformed[&output.name];
                    let new_width = (width as f64 / output.scale) as i32;
                    let translation_x = new_width - width;

                    for other in &sorted_by_x {
                        let (_, other_y, _, other_height) = transformed[&other.name];
                        let is_to_the_right = other.x >= output.x + width;
                        let overlaps_y = other_y <= output.y + height && other_y + other_height >= output.y;

                        if is_to_the_right && overlaps_y {
                            *translations_x.entry(other.name.clone()).or_default() += translation_x;
                        }
                    }

                    transformed.get_mut(&output.name).unwrap().2 = new_width;
                }
            }
            for (name, shift) in translations_x {
                transformed.get_mut(&name).unwrap().0 += shift;
            }

            // Repeat the same thing for the y-axis. Meaning, all outputs
            // are sorted by ascending y-coordinate and then their transformations
            // are applied on the y-axis. If multiple outputs overlap, the one which
            // is further down is moved further down such that the outputs no longer
            // overlap.
            let mut sorted_by_y = outputs.iter().collect::<Vec<_>>();
            sorted_by_y.sort_by_key(|o| o.y);

            let mut translations_y = HashMap::<String, i32>::new();
            for output in &sorted_by_y {
                if output.is_scaled() {
                    let (_, _, width, height) = transformed[&output.name];
                    let new_height = (height as f64 / output.scale) as i32;
                    let translation_y = new_height - height;

                    for other in &sorted_by_y {
                        let (other_x, _, other_width, _) = transformed[&other.name];
                        let is_below = other.y >= output.y + height;
                        let overlaps_x = other_x <= output.x + width && other_x + other_width >= output.x;

                        if is_below && overlaps_x {
                            *translations_y.entry(other.name.clone()).or_default() += translation_y;
                        }
                    }

                    transformed.get_mut(&output.name).unwrap().3 = new_height;
                }
            }
            for (name, shift) in translations_y {
                transformed.get_mut(&name).unwrap().1 += shift;
            }

            let geometry = Self::calculate_geometry(transformed.values());
            Self(transformed, geometry)
        }

        /// Create a new [`OutputArea`] without applying the scaling of the [`Output`]s
        /// to their geometry.
        pub fn new_without_scaling(outputs: &[Output]) -> Self {
            let outputs = outputs
                .iter()
                .map(|output| {
                    let (width, height) = output.transformed_dimensions();
                    let &Output { name, x, y, .. } = &output;
                    (name.clone(), (*x, *y, width, height))
                })
                .collect::<HashMap<_, _>>();

            let geometry = Self::calculate_geometry(outputs.values());
            Self(outputs, geometry)
        }

        /// Geometry of the rectangular space spanned by all outputs
        /// of the [`OutputArea`].
        pub fn geometry(&self) -> Geometry {
            self.1
        }

        /// Geometry of an output of the [`OutputArea`].
        pub fn output(&self, output: &Output) -> Geometry {
            *self.0.get(&output.name).expect("output should exist")
        }

        /// Tuple containing the offsets which need to be applied to all
        /// outputs to have (0,0) at the top-right corner of the [`OutputArea`].
        pub fn offsets(&self) -> (i32, i32) {
            let (x, y, _, _) = self.1;
            (-x, -y)
        }

        /// Get the overall rectangular geometry of the area spanned by
        /// all outputs of the [`OutputArea`].
        fn calculate_geometry(outputs: Values<String, Geometry>) -> Geometry {
            let min_x = outputs.clone().min_by_key(|(x, _, _, _)| x).map(|(x, _, _, _)| *x).unwrap();
            let min_y = outputs.clone().min_by_key(|(_, y, _, _)| y).map(|(_, y, _, _)| *y).unwrap();
            let max_x = outputs.clone().max_by_key(|(x, _, width, _)| x + width).map(|(x, _, width, _)| x + width).unwrap();
            let max_y = outputs.max_by_key(|(_, y, _, height)| y + height).map(|(_, y, _, height)| y + height).unwrap();

            (min_x, min_y, max_x - min_x, max_y - min_y)
        }
    }
}

pub struct OutputsView<'a> {
    config: &'a Config,
    manager: Arc<OutputManager>,
    area: OutputArea,
}

impl<'a> OutputsView<'a> {
    pub fn new(connection: &'a Connection, config: &'a Config) -> Result<Self, String> {
        let manager = OutputManager::new(connection)
            .map(Arc::new)
            .map_err(|err| format!("unable to create new output manager from connection: {err}"))?;

        log::debug!("got outputs {:#?}", manager.outputs);

        let area = if config.outputs.respect_output_scaling {
            OutputArea::new_with_scaling(&manager.outputs)
        } else {
            OutputArea::new_without_scaling(&manager.outputs)
        };

        Ok(OutputsView { config, manager, area })
    }
}

impl View for OutputsView<'_> {
    fn build(&self) -> ScrolledWindow {
        let container = Fixed::builder().hexpand(false).vexpand(false).build();
        let scrolled_window =
            ScrolledWindow::builder().child(&container).css_classes([self.config.classes.notebook_page.as_str()]).build();

        self.manager.outputs.iter().for_each(|output| {
            let output_card = OutputCard::new(output, self.config, &self.area, self.manager.clone());
            let card = match output_card.build() {
                Ok(card) => card,
                Err(err) => return log::error!("unable to build output card for output {}: {err}", output.name),
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
    output: &'a Output,
    config: &'a Config,
    manager: Arc<OutputManager>,
    area: &'a OutputArea,
}

impl<'a> OutputCard<'a> {
    fn new(output: &'a Output, config: &'a Config, area: &'a OutputArea, manager: Arc<OutputManager>) -> Self {
        Self { output, config, manager, area }
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
            .css_classes([self.config.classes.image_card.as_str(), self.config.classes.image_card_loading.as_str()])
            .build();

        let (area_min_x, area_min_y, area_width, area_height) = self.area.geometry();
        let (x, y, width, height) = self.area.output(self.output);

        if area_min_x != x {
            container.set_margin_start(self.config.outputs.spacing as i32);
        }
        if area_min_x + area_width != x + width {
            container.set_margin_end(self.config.outputs.spacing as i32);
        }
        if area_min_y != y {
            container.set_margin_top(self.config.outputs.spacing as i32);
        }
        if area_min_y + area_height != y + height {
            container.set_margin_bottom(self.config.outputs.spacing as i32);
        }
        container.append(picture);

        if self.config.outputs.show_label {
            let label = Label::builder()
                .max_width_chars(1)
                .label(&self.output.name)
                .ellipsize(gtk4::pango::EllipsizeMode::End)
                .single_line_mode(true)
                .css_classes([self.config.classes.image_label.as_str()])
                .hexpand(false)
                .build();

            container.append(&label);
        }

        container
    }

    fn build_card_container(&self, card: &Box) -> Button {
        let container = Button::builder().focusable(true).child(card).build();

        let gesture = GestureClick::new();
        gesture.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let clicks = self.config.windows.clicks;
        let name = &self.output.name;
        gesture.connect_released(clone!(
            #[strong]
            name,
            move |gesture, n, _, _| {
                if n as i64 == clicks as i64
                    && let Some(widget) = gesture.widget()
                {
                    widget
                        .activate_action("win.select", Some(&format!("screen:{name}").to_variant()))
                        .expect("select action should be registered on the window")
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
        let (_, _, area_width, area_height) = self.area.geometry();
        let (x, y, width, height) = self.area.output(self.output);
        let (offset_x, offset_y) = self.area.offsets();

        let (area_width, area_height) = (area_width as f64, area_height as f64);
        let (x, y, width, height) = (x as f64, y as f64, width as f64, height as f64);
        let (offset_x, offset_y) = (offset_x as f64, offset_y as f64);

        container.add_tick_callback(clone!(
            #[strong]
            card,
            move |container, _| {
                let allocation = container.allocation();
                // listen to ticks until we have an allocation
                if allocation.width() == 0 || allocation.height() == 0 {
                    glib::ControlFlow::Continue
                } else {
                    let (allocation_width, allocation_height) = (allocation.width() as f64, allocation.height() as f64);

                    // Uniform scale factor that fits the area inside the container
                    // without stretching.
                    let scale = (allocation_width / area_width).min(allocation_height / area_height);

                    // Apply transformed dimensions to the card.
                    card.set_width_request((width * scale) as i32);
                    card.set_height_request((height * scale) as i32);

                    let px_offset_x = (allocation_width - area_width * scale).max(0.0) / 2.0;
                    let px_offset_y = (allocation_height - area_height * scale).max(0.0) / 2.0;

                    container.put(&card, px_offset_x + (offset_x + x) * scale, px_offset_y + (offset_y + y) * scale);
                    glib::ControlFlow::Break
                }
            }
        ));
    }

    fn request_frame(&self, tx: Sender<Image>) {
        let resize_size = self.config.image.resize_size;
        let manager = self.manager.clone();
        let name = &self.output.name;
        let output = self.output;

        tokio::spawn(clone!(
            #[strong]
            name,
            #[strong]
            output,
            #[to_owned]
            manager,
            async move {
                let buffer = match manager.to_owned().capture_output(&output.wl_output) {
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

                if tx.send(img).is_err() {
                    log::error!("unable to transmit image for name {name}: channel is closed");
                };
                log::debug!("transmitted image for output {name}");
            }
        ));
    }

    fn update_frame_lazily(&self, card: Box, picture: Picture, rx: Receiver<Image>) {
        let loading_class = self.config.classes.image_card_loading.clone();
        let name = self.output.name.clone();
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
