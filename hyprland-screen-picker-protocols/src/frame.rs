use wayland_client::{
    Connection, Dispatch, delegate_noop,
    protocol::{wl_buffer::WlBuffer, wl_registry, wl_shm::WlShm, wl_shm_pool::WlShmPool},
};

use crate::{
    buffer::Buffer,
    protocols::hyprland_toplevel_export_v1::{
        hyprland_toplevel_export_frame_v1::{self, HyprlandToplevelExportFrameV1},
        hyprland_toplevel_export_manager_v1::HyprlandToplevelExportManagerV1,
    },
};
use std::{boxed::Box, rc::Rc};

#[derive(Default, Debug)]
pub enum FrameStatus {
    #[default]
    Inactive,
    Active,
    FrameReady(Rc<Buffer>),
    Failed,
    FrameRequested(Rc<Buffer>),
    BufferDone(Rc<Buffer>),
    Error(Box<dyn std::error::Error>),
}

pub struct FrameManager {
    shm: Option<WlShm>,
    manager: Option<HyprlandToplevelExportManagerV1>,
    pub status: FrameStatus,
    connection: Connection,
}

impl FrameManager {
    /// setup a new frame manager which can be used to get one or more frames for windows
    pub fn new(connection: &Connection) -> Result<Self, Box<dyn std::error::Error>> {
        let display = connection.display();

        let mut event_queue = connection.new_event_queue();
        let handle = event_queue.handle();

        let mut manager = Self { status: FrameStatus::Inactive, shm: None, manager: None, connection: connection.clone() };

        display.get_registry(&handle, ());

        event_queue.roundtrip(&mut manager)?;

        if let None = manager.manager {
            return Err(Box::from("hyprland toplevel export manager v1 is not available"));
        }
        if let None = manager.shm {
            return Err(Box::from("wl shm is not available"));
        }

        Ok(manager)
    }

    /// capture a single frame buffer for a window handle
    pub fn capture_frame(&mut self, window_handle: u64) -> Result<Rc<Buffer>, Box<dyn std::error::Error>> {
        let &FrameStatus::Inactive = &self.status else {
            return Err(Box::from("frame manager is not in inactive status"));
        };

        let Some(hl_manager) = &self.manager else {
            return Err(Box::from("hyprland toplevel export manager is not available"));
        };

        self.status = FrameStatus::Active;

        let mut event_queue = self.connection.new_event_queue();
        let handle = event_queue.handle();
        let hl_frame = hl_manager.capture_toplevel(0, window_handle as u32, &handle, ());
        loop {
            if let Err(err) = event_queue.blocking_dispatch(self) {
                self.status = FrameStatus::Inactive;
                Err(err)?;
            }
            match &self.status {
                FrameStatus::FrameReady(buffer) => {
                    let buffer = buffer.clone();
                    hl_frame.destroy();
                    self.status = FrameStatus::Inactive;
                    return Ok(buffer);
                }
                FrameStatus::BufferDone(buffer) => {
                    hl_frame.copy(&buffer.buffer, 1);
                    self.status = FrameStatus::FrameRequested(buffer.clone());
                }
                FrameStatus::Error(err) => {
                    let err = Box::from(format!("error during frame capture: {err}"));
                    self.status = FrameStatus::Inactive;
                    hl_frame.destroy();
                    return Err(err);
                }
                FrameStatus::Failed => {
                    self.status = FrameStatus::Inactive;
                    hl_frame.destroy();
                    return Err(Box::from("frame copy failed"));
                }
                _ => {}
            }
        }
    }

    /// destroy the internal objects of the frame manager
    pub fn destroy(&mut self) {
        if let Some(hl_manager) = &self.manager {
            hl_manager.destroy();
            self.manager = None;
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for FrameManager {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        handle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global { name, interface, version } => match interface.as_str() {
                "wl_shm" => {
                    let shm: WlShm = registry.bind(name, version, handle, ());
                    state.shm = Some(shm);
                }
                "hyprland_toplevel_export_manager_v1" => {
                    let manager: HyprlandToplevelExportManagerV1 = registry.bind(name, version, handle, ());
                    state.manager = Some(manager);
                }
                _ => {}
            },
            _ => {}
        }
    }
}

impl Dispatch<HyprlandToplevelExportFrameV1, ()> for FrameManager {
    fn event(
        state: &mut Self,
        _proxy: &HyprlandToplevelExportFrameV1,
        event: <HyprlandToplevelExportFrameV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            hyprland_toplevel_export_frame_v1::Event::Buffer { format, width, height, stride } => {
                let Ok(format) = format.into_result() else {
                    state.status = FrameStatus::Error(Box::from("buffer format was not valid enum"));
                    return;
                };
                if let Some(shm) = &state.shm {
                    match Buffer::new(shm, width, height, stride, format, qhandle, ()) {
                        Ok(buffer) => state.status = FrameStatus::BufferDone(Rc::new(buffer)),
                        Err(err) => state.status = FrameStatus::Error(Box::from(format!("unable to create buffer: {err}"))),
                    }
                } else {
                    state.status = FrameStatus::Error(Box::from("buffer event is called without having shm"));
                }
            }
            hyprland_toplevel_export_frame_v1::Event::Damage { .. } => {}
            hyprland_toplevel_export_frame_v1::Event::Flags { flags } => {
                // todo!("parse flags")
            }
            hyprland_toplevel_export_frame_v1::Event::Ready { .. } => {
                if let FrameStatus::FrameRequested(buffer) = &state.status {
                    state.status = FrameStatus::FrameReady(buffer.clone())
                } else {
                    state.status = FrameStatus::Error(Box::from("received frame ready without having requested a frame"))
                }
            }
            hyprland_toplevel_export_frame_v1::Event::Failed => state.status = FrameStatus::Failed,
            hyprland_toplevel_export_frame_v1::Event::LinuxDmabuf { .. } => {}
            hyprland_toplevel_export_frame_v1::Event::BufferDone => {}
        }
    }
}

delegate_noop!(FrameManager: ignore WlShm);
delegate_noop!(FrameManager: ignore WlShmPool);
delegate_noop!(FrameManager: ignore WlBuffer);
delegate_noop!(FrameManager: ignore HyprlandToplevelExportManagerV1);
