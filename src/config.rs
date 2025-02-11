use std::path::{Path, PathBuf};

use log::{error, warn};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, JsonSchema)]
#[serde(default)]
pub struct Config {
    #[serde(skip_deserializing)]
    #[schemars(skip)]
    path: PathBuf,
    /// all config related to the application window
    pub window: WindowConfig,
    /// paths to all stylesheets which should be loaded
    ///
    /// the paths are relative to the location of the config file
    pub stylesheets: Vec<String>,
    /// hide the token restore checkbox and use the default value instead
    pub hide_token_restore: bool,
    /// notebook page which is selected by default
    pub default_page: Page,
    /// all config related to images
    pub image: ImageConfig,
    /// config for customizing widget css classes
    pub classes: ClassesConfig,
    /// config related to the region page
    pub region: RegionConfig,
    /// config related to the windows page
    pub windows: WindowsConfig,
    /// config related to the outputs page
    pub outputs: OutputsConfig,
}

impl Config {
    pub fn new(path_str: &String) -> Self {
        let path = Path::new(path_str);
        if path.exists() {
            let str = std::fs::read_to_string(path).unwrap_or_default();
            match serde_yaml::from_str(str.as_str()) {
                Ok(config) => Self { path: path.to_path_buf(), ..config },
                Err(err) => {
                    error!("invalid config file at {path_str}: {err}");
                    std::process::exit(1)
                }
            }
        } else {
            warn!("missing config file at {path_str}, using default instead!");
            Self::default()
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn directory(&self) -> &Path {
        &self.path.parent().unwrap_or(self.path().as_path())
    }

    /// Expand `$HOME` and `~` at beginning of a path to
    /// current user home directory if resolvable
    fn expand_path(path_str: &String) -> Option<PathBuf> {
        if !path_str.starts_with("~") && !path_str.starts_with("$HOME") {
            Some(Path::new(path_str).to_path_buf())
        } else if path_str == "~" || path_str == "$HOME" {
            dirs::home_dir()
        } else {
            dirs::home_dir().map(|home| {
                if home == Path::new("/") {
                    let without = path_str.replace("$HOME", "").replace("~", "");
                    Path::new(&without).to_path_buf()
                } else {
                    let home_str = home.to_str().unwrap_or_default();
                    let without = path_str.replace("$HOME", home_str).replace("~", home_str);
                    Path::new(&without).to_path_buf()
                }
            })
        }
    }

    /// Resolve relative paths to position of config file
    /// and expand `$HOME` and `~` to user home directory
    pub fn resolve_path(&self, path_str: &String) -> PathBuf {
        let path = match Self::expand_path(path_str) {
            Some(path) => path,
            None => {
                warn!("unable to resolve user home directory");
                Path::new(path_str).to_path_buf()
            }
        };

        if path.is_relative() {
            let full = self.directory().join(path);
            full.canonicalize().unwrap_or(full)
        } else {
            path
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            path: dirs::home_dir().unwrap_or(Path::new("/").to_path_buf()),
            window: WindowConfig::default(),
            stylesheets: Vec::default(),
            image: ImageConfig::default(),
            classes: ClassesConfig::default(),
            region: RegionConfig::default(),
            outputs: OutputsConfig::default(),
            windows: WindowsConfig::default(),
            hide_token_restore: false,
            default_page: Page::default()
        }
    }
}

#[derive(Deserialize, Debug, Clone, JsonSchema)]
#[schemars(rename = "Window config")]
#[serde(default)]
pub struct WindowConfig {
    /// target width of the application window
    pub width: i32,
    /// target height of the application window
    pub height: i32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self { width: 1000, height: 500 }
    }
}

#[derive(Deserialize, Debug, Clone, JsonSchema)]
#[schemars(rename = "Image config")]
#[serde(default)]
pub struct ImageConfig {
    /// internally downscale every image to this height
    ///
    /// if the image's height is already smaller than this height, nothing happens
    pub resize_size: u32,
    /// target height of the widget containing the image
    pub widget_size: i32,
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self { resize_size: 200, widget_size: 150 }
    }
}

#[derive(Deserialize, Debug, Clone, JsonSchema)]
#[schemars(rename = "Classes config")]
#[serde(default)]
pub struct ClassesConfig {
    /// class applied to the application window
    pub window: String,
    /// class applied to the card holding the image and label
    pub image_card: String,
    /// class applied to the image widget
    pub image: String,
    /// class applied to the image label widget
    pub image_label: String,
    /// class applied to the notebook widget
    pub notebook: String,
    /// class applied to the label of the notebook tabs
    pub tab_label: String,
    /// class applied to the container of a single page of the notebook
    pub notebook_page: String,
    /// class applied to the button which triggers the region selection
    pub region_button: String,
    /// class applied to the button containing the session restore checkbox and label
    pub restore_button: String,
}

impl Default for ClassesConfig {
    fn default() -> Self {
        Self {
            window: String::from("window"),
            image_card: String::from("card"),
            image: String::from("image"),
            image_label: String::from("image-label"),
            notebook: String::from("notebook"),
            tab_label: String::from("tab-label"),
            notebook_page: String::from("page"),
            region_button: String::from("region-button"),
            restore_button: String::from("restore-button"),
        }
    }
}

#[derive(Deserialize, Debug, Clone, JsonSchema)]
#[schemars(rename = "Region config")]
#[serde(default)]
pub struct RegionConfig {
    /// command to use for the region selection
    ///
    /// the command should return a value in the following format:
    /// <output>@<x>,<y>,<w>,<h> (e.g. DP-3@2789,436,756,576)
    pub command: String,
}

impl Default for RegionConfig {
    fn default() -> Self {
        Self { command: String::from("slurp -f '%o@%x,%y,%w,%h'") }
    }
}

#[derive(Deserialize, Debug, Clone, JsonSchema)]
#[schemars(rename = "Outputs config")]
#[serde(default)]
pub struct OutputsConfig {
    /// minimum amount of cards per row
    pub min_per_row: u32,
    /// minimum amount of cards per row
    pub max_per_row: u32,
}

impl Default for OutputsConfig {
    fn default() -> Self {
        Self { min_per_row: 2, max_per_row: 2 }
    }
}

#[derive(Deserialize, Debug, Clone, JsonSchema)]
#[schemars(rename = "Windows config")]
#[serde(default)]
pub struct WindowsConfig {
    /// minimum amount of cards per row
    pub min_per_row: u32,
    /// minimum amount of cards per row
    pub max_per_row: u32,
}

impl Default for WindowsConfig {
    fn default() -> Self {
        Self { min_per_row: 3, max_per_row: 999 }
    }
}

#[derive(Deserialize, Debug, Clone, JsonSchema, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Page {
    #[default]
    Windows,
    Outputs,
    Region
}