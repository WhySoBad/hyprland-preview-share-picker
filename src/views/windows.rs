use std::sync::Arc;

use glib::{clone, variant::ToVariant};
use gtk4::{
    Box, FlowBox, FlowBoxChild, GestureClick, Label, Picture, ScrolledWindow,
    prelude::{BoxExt, EventControllerExt, FlowBoxChildExt, WidgetExt},
};
use hyprland::{
    data::{Client, Clients},
    shared::HyprData,
};
use hyprland_preview_share_picker_lib::{frame::FrameManager, image::Image, toplevel::Toplevel};
use tokio::sync::oneshot::{Receiver, Sender};
use wayland_client::Connection;

use crate::{config::Config, image::ImageExt};

use super::View;

pub struct WindowsView<'a> {
    toplevels: &'a [Toplevel],
    config: &'a Config,
    manager: Arc<FrameManager>,
    clients: Vec<Client>,
}

impl<'a> WindowsView<'a> {
    pub fn new(connection: &'a Connection, toplevels: &'a [Toplevel], config: &'a Config) -> Result<Self, String> {
        let manager = FrameManager::new(connection)
            .map(Arc::new)
            .map_err(|err| format!("unable to create new frame manager from connection: {err}"))?;
        let clients = Clients::get()
            .map(|clients| clients.into_iter().collect::<Vec<_>>())
            .map_err(|err| format!("unable to get clients from hyprland socket: {err}"))?;

        Ok(Self { toplevels, config, manager, clients })
    }
}

impl View for WindowsView<'_> {
    fn build(&self) -> ScrolledWindow {
        let container = FlowBox::builder()
            .vexpand(false)
            .homogeneous(false)
            .row_spacing(12)
            .column_spacing(12)
            .orientation(gtk4::Orientation::Horizontal)
            .homogeneous(true)
            .min_children_per_line(self.config.windows.min_per_row)
            .max_children_per_line(self.config.windows.max_per_row)
            .build();
        let scrolled_window =
            ScrolledWindow::builder().child(&container).css_classes([self.config.classes.notebook_page.as_str()]).build();

        self.toplevels.iter().for_each(|toplevel| {
            log::debug!("attempting to capture frame for toplevel {}", toplevel.id);
            // this method is kindof bad since multiple windows could have the same class and title but afaik there is no clean
            // way to get a hyprland window address for a wayland toplevel id
            log::debug!("toplevel = {toplevel:?}");
            let client = match self.clients.iter().find(|c| c.class.eq(&toplevel.class) && c.title.eq(&toplevel.title)) {
                Some(client) => client,
                None => return log::error!("unable to find hyprland client which matches toplevel class and title"),
            };

            let window_card = WindowCard::new(toplevel, client, self.config, self.manager.clone());
            let card = match window_card.build() {
                Ok(card) => card,
                Err(err) => return log::error!("unable to build window card for toplevel {}: {err}", toplevel.id),
            };

            container.insert(&card, 0);
        });

        scrolled_window
    }

    fn label(&self) -> Label {
        Label::builder().css_classes([self.config.classes.tab_label.as_str()]).label("Windows").build()
    }
}

struct WindowCard<'a> {
    toplevel: &'a Toplevel,
    client: &'a Client,
    config: &'a Config,
    manager: Arc<FrameManager>,
}

impl<'a> WindowCard<'a> {
    pub fn new(toplevel: &'a Toplevel, client: &'a Client, config: &'a Config, manager: Arc<FrameManager>) -> Self {
        WindowCard { toplevel, client, config, manager }
    }

    pub fn build(self) -> Result<FlowBoxChild, String> {
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
            .valign(gtk4::Align::Center)
            .height_request(self.config.image.widget_size)
            .content_fit(gtk4::ContentFit::Contain)
            .css_classes([self.config.classes.image.as_str()])
            .build()
    }

    fn build_card(&self, picture: &Picture) -> Box {
        let container = Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .vexpand(false)
            .hexpand(false)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Start)
            .css_classes([self.config.classes.image_card.as_str(), self.config.classes.image_card_loading.as_str()])
            .build();

        let label = Label::builder()
            .max_width_chars(1)
            .label(self.toplevel.title.as_str())
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .single_line_mode(true)
            .css_classes([self.config.classes.image_label.as_str()])
            .hexpand(false)
            .build();

        container.append(picture);
        container.append(&label);
        container
    }

    fn build_card_container(&self, card: &Box) -> FlowBoxChild {
        let container = FlowBoxChild::builder().halign(gtk4::Align::Fill).valign(gtk4::Align::Fill).child(card).build();

        let gesture = GestureClick::new();
        let clicks = self.config.windows.clicks;
        let id = self.toplevel.id;
        gesture.connect_released(move |gesture, n, _, _| {
            if n as i64 == clicks as i64 {
                if let Some(widget) = gesture.widget() {
                    widget
                        .activate_action("win.select", Some(&format!("window:{id}").to_variant()))
                        .expect("select action should be registered on the window")
                }
            }
        });
        container.add_controller(gesture);
        container.connect_activate(move |child| {
            child
                .activate_action("win.select", Some(&format!("window:{id}").to_variant()))
                .expect("select action should be registered on the window")
        });
        container
    }

    fn request_frame(&self, tx: Sender<Image>) {
        let handle_str = &format!("{}", self.client.address)[2..];
        let handle = u64::from_str_radix(handle_str, 16).expect("should be valid u64");
        let id = self.toplevel.id;
        let resize_size = self.config.image.resize_size;
        let manager = self.manager.clone();

        tokio::spawn(clone!(
            #[to_owned]
            manager,
            async move {
                let buffer = match manager.to_owned().capture_frame(handle) {
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
    }

    fn update_frame_lazily(&self, card: Box, picture: Picture, rx: Receiver<Image>) {
        let id = self.toplevel.id;
        let loading_class = self.config.classes.image_card_loading.clone();
        glib::spawn_future_local(async move {
            let img = match rx.await {
                Ok(img) => img,
                Err(err) => {
                    log::error!("unable to receive image for toplevel {id}: {err}");
                    card.remove_css_class(&loading_class);
                    return;
                }
            };

            let pixbuf = match img.into_pixbuf() {
                Ok(pixbuf) => pixbuf,
                Err(err) => return log::error!("unable to create pixbuf for toplevel {id} image: {err}"),
            };

            picture.set_pixbuf(Some(&pixbuf));
            card.remove_css_class(&loading_class);
        });
    }
}
