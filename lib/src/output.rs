use std::sync::{Arc, Mutex, Weak};

use derive_builder::Builder;
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_output::{self, Transform, WlOutput};
use wayland_client::protocol::wl_registry;
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::{Connection, Dispatch, EventQueue, delegate_noop, event_created_child};
use wayland_protocols_wlr::output_management::v1::client::zwlr_output_head_v1::{self, ZwlrOutputHeadV1};
use wayland_protocols_wlr::output_management::v1::client::zwlr_output_manager_v1::{self, ZwlrOutputManagerV1};
use wayland_protocols_wlr::output_management::v1::client::zwlr_output_mode_v1::{self, ZwlrOutputModeV1};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use crate::Frame;
use crate::buffer::Buffer;
use crate::error::Error;

#[derive(Clone, Debug, Builder)]
pub struct Output {
    /// Unique name of the output.
    pub name: String,
    /// Human-readable description of the output.
    pub description: String,
    /// Scale of the output in global compositor space.
    pub scale: f64,
    /// Position on the x-axis in global compositor space.
    pub x: i32,
    /// Position on the y-axis in global compositor space.
    pub y: i32,
    /// Width of the output in millimeters.
    pub physical_width: i32,
    /// Height of the output in millimeters.
    pub physical_height: i32,
    /// Width of the output in pixels.
    pub width: i32,
    /// Height of the output in pixels.
    pub height: i32,
    /// Transformation applied to the output.
    pub transform: Transform,
    /// Underlying `wl_output` object.
    pub wl_output: WlOutput,
}

impl PartialEq for Output {
    fn eq(&self, other: &Self) -> bool {
        self.wl_output == other.wl_output
    }
}

impl Output {
    /// Get a tuple containing the (width, height) dimensions
    /// of the output after applying the output's transform.
    pub fn transformed_dimensions(&self) -> (i32, i32) {
        match self.transform {
            Transform::Normal | Transform::_180 | Transform::Flipped | Transform::Flipped180 => (self.width, self.height),
            Transform::_90 | Transform::_270 | Transform::Flipped90 | Transform::Flipped270 => (self.height, self.width),
            transform => {
                log::error!("received unsupported output transform: {transform:?}");
                (self.width, self.height)
            }
        }
    }

    /// Get whether a non-one scaling factor is applied to the output.
    pub fn is_scaled(&self) -> bool {
        self.scale != 1.0
    }
}

#[derive(Clone)]
pub struct OutputManager {
    shm: Option<WlShm>,
    screencopy_manager: Option<ZwlrScreencopyManagerV1>,
    output_manager: Option<ZwlrOutputManagerV1>,
    output_manager_done: bool,
    pub outputs: Vec<Output>,
    pending_outputs: Vec<(WlOutput, Option<String>)>,
    pending_heads: Vec<(ZwlrOutputHeadV1, OutputBuilder, Option<ZwlrOutputModeV1>)>,
    modes: Vec<(ZwlrOutputModeV1, i32, i32)>,
    connection: Connection,
}

impl OutputManager {
    /// setup a new output manager which can be used to capture one or more frames of outputs or of selected regions
    pub fn new(connection: &Connection) -> Result<Self, Error> {
        let display = connection.display();

        let mut event_queue = connection.new_event_queue();
        let handle = event_queue.handle();

        let mut manager = Self {
            shm: None,
            screencopy_manager: None,
            output_manager: None,
            output_manager_done: false,
            outputs: Vec::new(),
            pending_outputs: Vec::new(),
            pending_heads: Vec::new(),
            connection: connection.clone(),
            modes: Vec::new(),
        };

        display.get_registry(&handle, ());

        event_queue.roundtrip(&mut manager).map_err(|err| Error::WaylandDispatch(err))?;

        if let None = manager.screencopy_manager {
            Err(Error::ProtocolNotAvailable(std::any::type_name::<ZwlrScreencopyManagerV1>()))?
        }
        if let None = manager.output_manager {
            Err(Error::ProtocolNotAvailable(std::any::type_name::<ZwlrOutputManagerV1>()))?
        }
        if let None = manager.shm {
            Err(Error::ProtocolNotAvailable(std::any::type_name::<WlShm>()))?
        }

        event_queue.roundtrip(&mut manager).map_err(|err| Error::WaylandDispatch(err))?;

        Ok(manager)
    }

    /// capture a single frame buffer of an output
    pub fn capture_output(&mut self, output: &WlOutput) -> Result<Buffer, Error> {
        let Some(zwlr_manager) = &self.screencopy_manager else {
            Err(Error::ProtocolNotAvailable(std::any::type_name::<ZwlrScreencopyManagerV1>()))?
        };

        let frame = Arc::new(Mutex::new(Frame::default()));
        let mut event_queue = self.connection.new_event_queue();
        let handle = event_queue.handle();
        let zwlr_frame = zwlr_manager.capture_output(0, output, &handle, Arc::downgrade(&frame));
        self.finish_capture(frame, zwlr_frame, &mut event_queue)
    }

    /// capture a selected region of an output
    pub fn capture_output_region(
        &mut self,
        output: &WlOutput,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<Buffer, Error> {
        let Some(zwlr_manager) = &self.screencopy_manager else {
            Err(Error::ProtocolNotAvailable(std::any::type_name::<ZwlrScreencopyManagerV1>()))?
        };

        let frame = Arc::new(Mutex::new(Frame::default()));
        let mut event_queue = self.connection.new_event_queue();
        let handle = event_queue.handle();
        let zwlr_frame = zwlr_manager.capture_output_region(0, output, x, y, width, height, &handle, Arc::downgrade(&frame));
        self.finish_capture(frame, zwlr_frame, &mut event_queue)
    }

    fn finish_capture(
        &mut self,
        frame: Arc<Mutex<Frame>>,
        zwlr_frame: ZwlrScreencopyFrameV1,
        event_queue: &mut EventQueue<OutputManager>,
    ) -> Result<Buffer, Error> {
        loop {
            if let Err(err) = event_queue.blocking_dispatch(self) {
                Err(Error::WaylandDispatch(err))?;
            }
            let frame = frame.clone();
            let mut current = frame.lock().expect("lock should not be poisoned");
            match (current.ready, current.requested, &current.error, &current.buffer) {
                (_, _, Some(_), _) | (true, _, _, Some(_)) => {
                    zwlr_frame.destroy();
                    break;
                }
                (false, false, _, Some(buffer)) => {
                    zwlr_frame.copy(&buffer.buffer);
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

    /// For all `pending_outputs` check whether they can successfully be finalized.
    /// If that's the case, the pending output is inserted into `outputs`.
    fn build_outputs(&mut self) {
        if !self.output_manager_done {
            return;
        };

        self.pending_outputs.retain(|(wl_output, name)| {
            let Some(name) = name else { return true };
            let Some(idx) = self
                .pending_heads
                .iter()
                .position(|(_, output, _)| output.name.as_ref().is_some_and(|output_name| output_name == name))
            else {
                return true;
            };
            let (_, _, current_mode) = &self.pending_heads[idx];
            let Some((_, width, height)) =
                current_mode.as_ref().and_then(|current_mode| self.modes.iter().find(|(mode, _, _)| mode == current_mode))
            else {
                return true;
            };

            let (_, mut output, _) = self.pending_heads.remove(idx);
            output.width(*width);
            output.height(*height);
            output.wl_output(wl_output.clone());

            self.outputs.push(output.build().expect("all fields should be populated"));

            false
        });
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for OutputManager {
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
                "zwlr_screencopy_manager_v1" => {
                    let manager: ZwlrScreencopyManagerV1 = registry.bind(name, version, handle, ());
                    state.screencopy_manager = Some(manager);
                }
                "zwlr_output_manager_v1" => {
                    let manager: ZwlrOutputManagerV1 = registry.bind(name, version, handle, ());
                    state.output_manager = Some(manager);
                }
                "wl_output" => {
                    let output: WlOutput = registry.bind(name, version, handle, ());
                    state.pending_outputs.push((output, None));
                }
                _ => {}
            },
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for OutputManager {
    fn event(
        state: &mut Self,
        proxy: &wl_output::WlOutput,
        event: <wl_output::WlOutput as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        let Some((_, name_opt)) = state.pending_outputs.iter_mut().find(|(output, _)| output == proxy) else { return };

        match event {
            wl_output::Event::Name { name } => *name_opt = Some(name),
            wl_output::Event::Done => state.build_outputs(),
            _ => {}
        }
    }
}

impl Dispatch<ZwlrOutputManagerV1, ()> for OutputManager {
    event_created_child!(OutputManager, ZwlrOutputManagerV1, [
        zwlr_output_manager_v1::EVT_HEAD_OPCODE => (ZwlrOutputHeadV1, ())
    ]);

    fn event(
        state: &mut Self,
        _proxy: &ZwlrOutputManagerV1,
        event: <ZwlrOutputManagerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            zwlr_output_manager_v1::Event::Head { head } => state.pending_heads.push((head, OutputBuilder::default(), None)),
            zwlr_output_manager_v1::Event::Done { .. } => {
                // All output data should be sent.
                state.output_manager_done = true;
                state.build_outputs();
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrOutputHeadV1, ()> for OutputManager {
    event_created_child!(OutputManager, ZwlrOutputHeadV1, [
        zwlr_output_head_v1::EVT_MODE_OPCODE => (ZwlrOutputModeV1, ())
    ]);

    fn event(
        state: &mut Self,
        proxy: &ZwlrOutputHeadV1,
        event: <ZwlrOutputHeadV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        let Some((_, output, mode_opt)) = state.pending_heads.iter_mut().find(|(head, _, _)| head == proxy) else { return };

        match event {
            zwlr_output_head_v1::Event::Name { name } => {
                output.name(name);
            }
            zwlr_output_head_v1::Event::Description { description } => {
                output.description(description);
            }
            zwlr_output_head_v1::Event::PhysicalSize { width, height } => {
                output.physical_width(width);
                output.physical_height(height);
            }
            zwlr_output_head_v1::Event::CurrentMode { mode } => *mode_opt = Some(mode),
            zwlr_output_head_v1::Event::Position { x, y } => {
                output.x(x);
                output.y(y);
            }
            zwlr_output_head_v1::Event::Transform { transform } => match transform.into_result() {
                Ok(transform) => {
                    output.transform(transform);
                }
                Err(err) => log::error!("received invalid monitor transform: {err}"),
            },
            zwlr_output_head_v1::Event::Scale { scale } => {
                output.scale(scale);
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrOutputModeV1, ()> for OutputManager {
    fn event(
        state: &mut Self,
        proxy: &ZwlrOutputModeV1,
        event: <ZwlrOutputModeV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        // We're only interested in the size event.
        if let zwlr_output_mode_v1::Event::Size { width, height } = event {
            state.modes.push((proxy.clone(), width, height));
            state.build_outputs();
        }
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, Weak<Mutex<Frame>>> for OutputManager {
    fn event(
        state: &mut Self,
        _proxy: &ZwlrScreencopyFrameV1,
        event: <ZwlrScreencopyFrameV1 as wayland_client::Proxy>::Event,
        data: &Weak<Mutex<Frame>>,
        _conn: &wayland_client::Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        let Some(data) = data.upgrade() else { return };
        let mut frame = data.lock().expect("lock should not be poisoned");
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer { format, width, height, stride } => {
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
            zwlr_screencopy_frame_v1::Event::Flags { .. } => {}
            zwlr_screencopy_frame_v1::Event::Ready { .. } => {
                frame.ready = true;
            }
            zwlr_screencopy_frame_v1::Event::Failed => frame.error = Some(Error::Failed),
            zwlr_screencopy_frame_v1::Event::Damage { .. } => {}
            zwlr_screencopy_frame_v1::Event::LinuxDmabuf { .. } => {}
            zwlr_screencopy_frame_v1::Event::BufferDone => {}
            _ => {}
        }
    }
}

delegate_noop!(OutputManager: ignore WlShm);
delegate_noop!(OutputManager: ignore WlShmPool);
delegate_noop!(OutputManager: ignore WlBuffer);
delegate_noop!(OutputManager: ignore ZwlrScreencopyManagerV1);
