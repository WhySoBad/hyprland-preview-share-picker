use wayland_client;
use wayland_client::protocol::*;

pub mod __interfaces {
    use wayland_client::protocol::__interfaces::*;
    use wayland_protocols_wlr::foreign_toplevel::v1::client::__interfaces::*;
    wayland_scanner::generate_interfaces!("./hyprland-protocols/protocols/hyprland-toplevel-export-v1.xml");
}
use self::__interfaces::*;
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1;

wayland_scanner::generate_client_code!("./hyprland-protocols/protocols/hyprland-toplevel-export-v1.xml");
