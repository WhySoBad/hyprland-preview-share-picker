use std::process::Command;

use glib::variant::ToVariant;
use gtk4::{
    Box, Button, Label, ScrolledWindow,
    prelude::{BoxExt, ButtonExt, WidgetExt},
};
use regex::Regex;

use crate::config::Config;

use super::View;

pub struct RegionView<'a> {
    config: &'a Config,
    regex: Regex,
    args: Vec<String>,
}

impl<'a> RegionView<'a> {
    pub fn new(config: &'a Config) -> Result<Self, String> {
        let args = shlex::split(&config.region.command)
            .ok_or(format!("received invalid region command: {}", config.region.command))?;
        let regex = Regex::new(r"^.+@-?\d+,-?\d+,\d+,\d+$").map_err(|err| format!("received invalid regex: {err}"))?;

        Ok(Self { config, regex, args })
    }
}

impl View for RegionView<'_> {
    fn build(&self) -> ScrolledWindow {
        let container = Box::builder()
            .css_classes([self.config.classes.notebook_page.as_str()])
            .orientation(gtk4::Orientation::Vertical)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .build();
        let scrolled_window = ScrolledWindow::builder().child(&container).build();

        let button =
            Button::builder().label("Select region").css_classes([self.config.classes.region_button.as_str()]).build();

        container.insert_child_after(&button, Option::<&Box>::None);

        let regex = self.regex.clone();
        let args = self.args.clone();
        button.connect_clicked(move |btn| {
            if let Some(root) = btn.root() {
                let mut command = Command::new(&args[0]);
                command.args(&args[1..]);
                log::info!("using {command:?} as region command");
                root.hide();

                let region_regex = regex.clone();
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
        });

        scrolled_window
    }

    fn label(&self) -> Label {
        Label::builder().css_classes([self.config.classes.tab_label.as_str()]).label("Region").build()
    }
}
