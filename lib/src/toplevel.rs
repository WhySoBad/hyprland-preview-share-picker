use regex::Regex;
use wayland_client::{Connection, Dispatch, event_created_child, protocol::wl_registry};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1::{self, ExtForeignToplevelHandleV1},
    ext_foreign_toplevel_list_v1::{EVT_TOPLEVEL_OPCODE, ExtForeignToplevelListV1},
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1}, zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1};

use crate::error::Error;

#[derive(Clone, Debug)]
pub struct WlToplevel {
    pub title: Option<String>,
    pub app_id: Option<String>,
    pub identifier: Option<String>,
}

impl WlToplevel {
    /// internal id of the toplevel in hyprland
    pub fn toplevel_id(&self) -> Option<u32> {
        self.identifier
            .as_ref()
            .and_then(|identifier| u32::from_str_radix(identifier.split("->").collect::<Vec<_>>()[0], 16).ok())
    }

    /// address of the hyprland window associated with the toplevel
    pub fn window_address(&self) -> Option<u64> {
        self.identifier
            .as_ref()
            .and_then(|identifier| u64::from_str_radix(identifier.split("->").collect::<Vec<_>>()[1], 16).ok())
    }
}

#[derive(Clone)]
pub struct ToplevelManager {
    manager: Option<ZwlrForeignToplevelManagerV1>,
    toplevels: Vec<WlToplevel>,
    initialized_toplevels: usize,
}

impl ToplevelManager {
    /// get all available toplevels listed by hyprland
    pub fn get_toplevels(connection: &Connection) -> Result<Vec<WlToplevel>, Error> {
        let display = connection.display();

        let mut event_queue = connection.new_event_queue();
        let handle = event_queue.handle();

        let mut manager = Self { manager: None, toplevels: Vec::new(), initialized_toplevels: 0 };

        display.get_registry(&handle, ());

        event_queue.roundtrip(&mut manager).map_err(|err| Error::WaylandDispatch(err))?;

        if let None = manager.manager {
            Err(Error::ProtocolNotAvailable(std::any::type_name::<ExtForeignToplevelListV1>()))?
        };

        // make a single roundrip to capture all toplevels
        event_queue.roundtrip(&mut manager).map_err(|err| Error::WaylandDispatch(err))?;

        Ok(manager.toplevels)
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for ToplevelManager {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        handle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global { name, interface, version } => {
                log::debug!("[{name}] {interface} v{version}");
                match interface.as_str() {
                    "zwlr_foreign_toplevel_manager_v1" => {
                        let manager: ZwlrForeignToplevelManagerV1 = registry.bind(name, version, handle, ());
                        state.manager = Some(manager);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for ToplevelManager {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrForeignToplevelManagerV1,
        _event: <ZwlrForeignToplevelManagerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
    }

    event_created_child!(ToplevelManager, ZwlrForeignToplevelManagerV1, [
        EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ())
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for ToplevelManager {
    fn event(
        state: &mut Self,
        _proxy: &ZwlrForeignToplevelHandleV1,
        event: <ZwlrForeignToplevelHandleV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            zwlr_foreign_toplevel_handle_v1::Event::Title { title } => {
                state.toplevels[state.initialized_toplevels].title = Some(title)
            },
            zwlr_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
                state.toplevels.push(WlToplevel { title: None, app_id: Some(app_id), identifier: None });
            },
            zwlr_foreign_toplevel_handle_v1::Event::OutputEnter { output } => {},
            zwlr_foreign_toplevel_handle_v1::Event::OutputLeave { output } => {},
            zwlr_foreign_toplevel_handle_v1::Event::State { state } => {},
            zwlr_foreign_toplevel_handle_v1::Event::Done => state.initialized_toplevels += 1,
            zwlr_foreign_toplevel_handle_v1::Event::Closed => {},
            zwlr_foreign_toplevel_handle_v1::Event::Parent { parent } => {},
            _ => {},
        }

        // match event {
        //     ext_foreign_toplevel_handle_v1::Event::Closed => {}
        //     ext_foreign_toplevel_handle_v1::Event::Done => state.initialized_toplevels += 1,
        //     ext_foreign_toplevel_handle_v1::Event::Title { title } => {
        //         state.toplevels[state.initialized_toplevels].title = Some(title)
        //     }
        //     ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
        //         state.toplevels[state.initialized_toplevels].app_id = Some(app_id)
        //     }
        //     ext_foreign_toplevel_handle_v1::Event::Identifier { identifier } => {
        //         log::debug!("identifier: {identifier}");
        //         state.toplevels.push(WlToplevel { title: None, app_id: None, identifier: Some(identifier) })
        //     }
        //     _ => {}
        // }
    }
}

#[derive(Clone, Debug)]
pub struct Toplevel {
    /// id of the wayland toplevel
    pub id: u32,
    /// class of the hyprland window the toplevel belongs to
    pub class: String,
    /// title of the hyprland window the toplevel belongs to
    pub title: String,
}

impl Toplevel {
    /// parse a window sharing list string as provided by the `XDPH_WINDOW_SHARING_LIST` env
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
                let id = split[0].parse::<u32>().ok()?;
                let class = split[1].to_string();
                let title = split[2].to_string();
                Some(Toplevel { id, class, title })
            })
            .collect::<Vec<_>>();

        return toplevels;
    }
}
