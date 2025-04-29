#[derive(Clone, Debug)]
pub struct Toplevel {
    /// id of the wayland toplevel
    pub id: u64,
    /// class of the hyprland window the toplevel belongs to
    pub class: String,
    /// title of the hyprland window the toplevel belongs to
    pub title: String,
    /// address of the window associated with the toplevel
    pub window_address: Option<u64>,
}

impl Toplevel {
    /// Parse a window sharing list string as provided by the `XDPH_WINDOW_SHARING_LIST` env
    /// which is set by the hyprland desktop portal
    ///
    /// see: https://github.com/hyprwm/xdg-desktop-portal-hyprland/blob/e09dfe2726c8008f983e45a0aa1a3b7416aaeb8a/src/shared/ScreencopyShared.cpp#L61
    pub fn parse_list(toplevel_list: &str) -> Vec<Toplevel> {
        let mut toplevels = Vec::new();

        let mut str = toplevel_list;
        while !str.is_empty() {
            let Some(id_sep_pos) = str.find("[HC>]") else {
                log::warn!("found no toplevel id separator");
                break;
            };
            let Ok(id) = str[0..id_sep_pos].parse::<u64>() else {
                log::warn!("toplevel id cannot be parsed to unsigned integer");
                break;
            };
            let Some(class_sep_pos) = str.find("[HT>]") else {
                log::warn!("found no toplevel class separator");
                break;
            };
            let class = str[id_sep_pos + 5..class_sep_pos].to_string();
            let Some(title_sep_pos) = str.find("[HE>]") else {
                log::warn!("found no toplevel title separator");
                break;
            };
            let title = str[class_sep_pos + 5..title_sep_pos].to_string();

            // for compatibility until the next hyprland release we support both, the [HA>] argument and it's absence
            let window_address = match str.find("[HA>]") {
                Some(window_sep_pos) => match str[title_sep_pos + 5..window_sep_pos].parse::<u64>() {
                    Ok(window_address) => {
                        str = &str[window_sep_pos + 5..];
                        Some(window_address)
                    }
                    Err(_) => {
                        log::warn!("window address cannot be parsed to unsigned integer");
                        break;
                    }
                },
                None => {
                    log::warn!("found no toplevel window separator");
                    str = &str[title_sep_pos + 5..];
                    None
                }
            };

            toplevels.push(Toplevel { id, class, title, window_address });
        }

        return toplevels;
    }
}
