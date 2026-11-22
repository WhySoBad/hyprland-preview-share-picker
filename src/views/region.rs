use std::process::Command;

use glib::variant::ToVariant;
use gtk4::prelude::{BoxExt, ButtonExt, WidgetExt};
use gtk4::{Box, Button, FlowBox, Label, ScrolledWindow, Separator};
use regex::Regex;

use super::View;
use crate::config::{Config, RegionPreset};

pub struct RegionView<'a> {
    config: &'a Config,
    regex: Regex,
    args: Vec<String>,
    presets: Vec<RegionPreset>,
}

impl<'a> RegionView<'a> {
    pub fn new(config: &'a Config) -> Result<Self, String> {
        let args = shlex::split(&config.region.command)
            .ok_or(format!("received invalid region command: {}", config.region.command))?;
        let regex = Regex::new(r"^.+@-?\d+,-?\d+,\d+,\d+$").map_err(|err| format!("received invalid regex: {err}"))?;

        let mut presets = config.region.presets.clone();
        if let Some(presets_command) = &config.region.presets_command {
            presets.extend(run_presets_command(presets_command));
        }

        let presets = presets
            .iter()
            .filter(|preset| {
                let valid = regex.is_match(preset.region.trim());
                if !valid {
                    log::warn!(
                        "skipping region preset '{}' with region '{}': expected '<output>@<x>,<y>,<w>,<h>'",
                        preset.name,
                        preset.region
                    );
                }
                valid
            })
            .map(|preset| RegionPreset { name: preset.name.clone(), region: preset.region.trim().to_string() })
            .collect();

        Ok(Self { config, regex, args, presets })
    }

    fn build_preset_button(&self, preset: &RegionPreset) -> Button {
        let button = Button::builder()
            .label(preset.name.as_str())
            .tooltip_text(preset.region.as_str())
            .css_classes([self.config.classes.region_button.as_str(), self.config.classes.region_preset_button.as_str()])
            .build();

        let region = preset.region.clone();
        button.connect_clicked(move |btn| {
            if let Some(root) = btn.root() {
                log::info!("using region preset {region}");
                root.activate_action("win.select", Some(&format!("region:{region}").to_variant()))
                    .expect("select action should be registered on the window");
            }
        });

        button
    }

    /// Append a section heading, unless it is configured empty.
    fn append_heading(&self, container: &Box, text: &str) {
        if !text.is_empty() {
            container
                .append(&Label::builder().label(text).css_classes([self.config.classes.region_heading.as_str()]).build());
        }
    }
}

/// Run the presets command and parse its `<name><TAB><region>` lines.
/// Any failure is logged and yields no presets, so the region page still opens.
fn run_presets_command(presets_command: &str) -> Vec<RegionPreset> {
    let Some(args) = shlex::split(presets_command).filter(|args| !args.is_empty()) else {
        log::error!("received invalid region presets command: {presets_command}");
        return Vec::new();
    };

    let mut command = Command::new(&args[0]);
    command.args(&args[1..]);
    log::info!("using {command:?} as region presets command");

    let output = match command.output() {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            log::error!("region presets command exited with {}", output.status);
            return Vec::new();
        }
        Err(err) => {
            log::error!("error whilst running region presets command: {err}");
            return Vec::new();
        }
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| match line.split_once('\t') {
            Some((name, region)) => Some(RegionPreset { name: name.trim().to_string(), region: region.to_string() }),
            None => {
                log::warn!("skipping region presets command line '{line}': expected '<name><TAB><region>'");
                None
            }
        })
        .collect()
}

impl View for RegionView<'_> {
    fn build(&self) -> ScrolledWindow {
        let container = Box::builder()
            .css_classes([self.config.classes.notebook_page.as_str()])
            .orientation(gtk4::Orientation::Vertical)
            .spacing(12)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .build();
        let scrolled_window = ScrolledWindow::builder().child(&container).build();

        let button =
            Button::builder().label("Select region").css_classes([self.config.classes.region_button.as_str()]).build();

        // Without presets the page is only the region selection button, as before.
        if !self.presets.is_empty() {
            self.append_heading(&container, &self.config.region.presets_heading);

            let presets = FlowBox::builder()
                .selection_mode(gtk4::SelectionMode::None)
                .column_spacing(6)
                .row_spacing(6)
                .halign(gtk4::Align::Center)
                .build();
            for preset in &self.presets {
                let preset_button = self.build_preset_button(preset);
                presets.insert(&preset_button, -1);
                // Keyboard focus goes straight to the buttons, not to their flow box cells.
                if let Some(cell) = preset_button.parent() {
                    cell.set_focusable(false);
                }
            }
            container.append(&presets);

            container.append(
                &Separator::builder()
                    .orientation(gtk4::Orientation::Horizontal)
                    .css_classes([self.config.classes.region_separator.as_str()])
                    .build(),
            );

            self.append_heading(&container, &self.config.region.selection_heading);
            button.set_halign(gtk4::Align::Center);
        }

        container.append(&button);

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
