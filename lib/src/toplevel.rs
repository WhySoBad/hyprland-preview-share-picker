use regex::Regex;

#[derive(Clone, Debug)]
pub struct Toplevel {
    /// id of the wayland toplevel
    pub id: u64,
    /// class of the hyprland window the toplevel belongs to
    pub class: String,
    /// title of the hyprland window the toplevel belongs to
    pub title: String,
}

impl Toplevel {
    /// Parse a window sharing list string as provided by the `XDPH_WINDOW_SHARING_LIST` env
    /// which is set by the hyprland desktop portal
    ///
    /// see: https://github.com/hyprwm/xdg-desktop-portal-hyprland/blob/e09dfe2726c8008f983e45a0aa1a3b7416aaeb8a/src/shared/ScreencopyShared.cpp#L61
    pub fn parse(toplevel_list: &str) -> Vec<Toplevel> {
        let regex = Regex::new(r"\[HC>\]|\[HT>\]").expect("should be valid regex");

        let toplevels = toplevel_list
            .split("[HE>]")
            .filter_map(|part| {
                let split = regex.split(part).collect::<Vec<_>>();
                if split.len() != 3 {
                    return None;
                }
                let id = split[0].parse::<u64>().ok()?;
                let class = split[1].to_string();
                let title = split[2].to_string();
                Some(Toplevel { id, class, title })
            })
            .collect::<Vec<_>>();

        return toplevels;
    }
}
