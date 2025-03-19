use hyprland::data::Monitor;

pub trait MonitorTransformExt {
    fn apply_transform(&mut self);
}

impl MonitorTransformExt for Monitor {
    fn apply_transform(&mut self) {
        match self.transform {
            hyprland::data::Transforms::Normal
            | hyprland::data::Transforms::Normal180
            | hyprland::data::Transforms::Flipped
            | hyprland::data::Transforms::Flipped180 => {}
            hyprland::data::Transforms::Normal90
            | hyprland::data::Transforms::Normal270
            | hyprland::data::Transforms::Flipped90
            | hyprland::data::Transforms::Flipped270 => {
                std::mem::swap(&mut self.height, &mut self.width);
            }
        }
    }
}
