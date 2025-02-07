use std::path::{Path, PathBuf};

use log::{error, warn};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Config {
    #[serde(skip_deserializing)]
    path: PathBuf,
    /// all config related to the application window
    pub window: WindowConfig,
    /// paths to all stylesheets which should be loaded
    ///
    /// the paths are relative to the location of the config file
    pub stylesheets: Vec<String>,
    /// all config related to images
    pub image: ImageConfig,
    /// config for customizing widget css classes
    pub classes: ClassesConfig,
}

impl Config {
    pub fn new(path_str: &String) -> Self {
        let path = Path::new(path_str);
        if path.exists() {
            let str = std::fs::read_to_string(path).unwrap_or_default();
            match toml::from_str(str.as_str()) {
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
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
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

#[derive(Deserialize, Debug, Clone)]
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
        Self { resize_size: 400, widget_size: 150 }
    }
}

#[derive(Deserialize, Debug, Clone)]
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
        }
    }
}
