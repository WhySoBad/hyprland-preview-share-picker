use wayland_client::{
    Connection, Dispatch, delegate_noop,
    protocol::{wl_buffer::WlBuffer, wl_registry, wl_shm::WlShm, wl_shm_pool::WlShmPool},
};

use crate::{
    Frame,
    buffer::Buffer,
    error::Error,
    protocols::hyprland_toplevel_export_v1::{
        hyprland_toplevel_export_frame_v1::{self, HyprlandToplevelExportFrameV1},
        hyprland_toplevel_export_manager_v1::HyprlandToplevelExportManagerV1,
    },
};
use std::sync::{Arc, Mutex, Weak};

pub struct FrameManager {
    shm: Option<WlShm>,
    manager: Option<HyprlandToplevelExportManagerV1>,
    connection: Connection,
}

impl FrameManager {
    /// setup a new frame manager which can be used to capture one or more frames for windows
    pub fn new(connection: &Connection) -> Result<Self, Error> {
        let display = connection.display();

        let mut event_queue = connection.new_event_queue();
        let handle = event_queue.handle();

        let mut manager = Self { shm: None, manager: None, connection: connection.clone() };

        display.get_registry(&handle, ());

        event_queue.roundtrip(&mut manager).map_err(|err| Error::WaylandDispatch(err))?;

        if let None = manager.manager {
            Err(Error::ProtocolNotAvailable(std::any::type_name::<HyprlandToplevelExportManagerV1>()))?
        }
        if let None = manager.shm {
            Err(Error::ProtocolNotAvailable(std::any::type_name::<WlShm>()))?
        }

        Ok(manager)
    }

    /// capture a single frame buffer of a window
    pub fn capture_frame(&mut self, window_handle: u64) -> Result<Buffer, Error> {
        log::debug!("attempting to capture frame for window {window_handle}");

        let Some(hl_manager) = &self.manager else {
            Err(Error::ProtocolNotAvailable(std::any::type_name::<HyprlandToplevelExportManagerV1>()))?
        };

        let frame = Arc::new(Mutex::new(Frame::default()));
        let mut event_queue = self.connection.new_event_queue();
        let handle = event_queue.handle();
        let hl_frame = hl_manager.capture_toplevel(0, window_handle as u32, &handle, Arc::downgrade(&frame));
        loop {
            if let Err(err) = event_queue.blocking_dispatch(self) {
                Err(Error::WaylandDispatch(err))?;
            }
            let frame = frame.clone();
            let mut current = frame.lock().expect("lock should not be poisoned");
            match (current.ready, current.requested, &current.error, &current.buffer) {
                (_, _, Some(_), _) | (true, _, _, Some(_)) => {
                    hl_frame.destroy();
                    break;
                }
                (false, false, _, Some(buffer)) => {
                    hl_frame.copy(&buffer.buffer, 1);
                    current.requested = true;
                }
                _ => continue,
            };
        }

        match Arc::into_inner(frame) {
            Some(frame) => {
                let frame = frame.into_inner().expect("lock should not be poisoned");
                if let Some(err) = frame.error {
                    return Err(err);
                }
                if let Some(buffer) = frame.buffer {
                    return Ok(buffer);
                } else {
                    unreachable!("we only exit the loop when buffer or error is some")
                }
            }
            None => unreachable!("we only exit the loop after waiting blockingly for all dispatchers"),
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

impl Dispatch<HyprlandToplevelExportFrameV1, Weak<Mutex<Frame>>> for FrameManager {
    fn event(
        state: &mut Self,
        _proxy: &HyprlandToplevelExportFrameV1,
        event: <HyprlandToplevelExportFrameV1 as wayland_client::Proxy>::Event,
        data: &Weak<Mutex<Frame>>,
        _conn: &Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        let Some(data) = data.upgrade() else {
            log::debug!(
                "dispatcher for HyprlandToplevelExportFrameV1 was called with event {event:?} but frame was already dropped"
            );
            return;
        };
        let mut frame = data.lock().expect("lock should not be poisoned");
        match event {
            hyprland_toplevel_export_frame_v1::Event::Buffer { format, width, height, stride } => {
                let format = match format.into_result() {
                    Ok(format) => format,
                    Err(err) => return frame.error = Some(Error::ProtocolInvalidEnum(err)),
                };
                if let Some(shm) = &state.shm {
                    match Buffer::new(shm, width, height, stride, format, qhandle, ()) {
                        Ok(buffer) => frame.buffer = Some(buffer),
                        Err(err) => frame.error = Some(err),
                    }
                } else {
                    frame.error = Some(Error::ProtocolNotAvailable(std::any::type_name::<WlShm>()));
                }
            }
            hyprland_toplevel_export_frame_v1::Event::Damage { .. } => {}
            hyprland_toplevel_export_frame_v1::Event::Flags { .. } => {}
            hyprland_toplevel_export_frame_v1::Event::Ready { .. } => {
                frame.ready = true;
            }
            hyprland_toplevel_export_frame_v1::Event::Failed => frame.error = Some(Error::Failed),
            hyprland_toplevel_export_frame_v1::Event::LinuxDmabuf { .. } => {}
            hyprland_toplevel_export_frame_v1::Event::BufferDone => {}
        }
    }
}

delegate_noop!(FrameManager: ignore WlShm);
delegate_noop!(FrameManager: ignore WlShmPool);
delegate_noop!(FrameManager: ignore WlBuffer);
delegate_noop!(FrameManager: ignore HyprlandToplevelExportManagerV1);
