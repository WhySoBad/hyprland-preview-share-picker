use std::sync::{Arc, Mutex, Weak};

use wayland_client::{
    Connection, Dispatch, EventQueue, delegate_noop,
    protocol::{
        wl_buffer::WlBuffer,
        wl_output::{self, Mode, Subpixel, Transform, WlOutput},
        wl_registry,
        wl_shm::WlShm,
        wl_shm_pool::WlShmPool,
    },
};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
};

use crate::{Frame, buffer::Buffer, error::Error};

#[derive(Debug, Clone)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub physical_width: i32,
    pub physical_height: i32,
    pub subpixel: Subpixel,
    pub make: String,
    pub model: String,
    pub transform: Transform,
}

#[derive(Debug, Clone)]
pub struct OutputMode {
    pub mode: Mode,
    pub width: i32,
    pub height: i32,
    pub refresh: i32,
}

#[derive(Default, Debug, Clone)]
pub struct Output {
    pub name: Option<String>,
    pub description: Option<String>,
    pub scale: Option<i32>,
    pub mode: Option<OutputMode>,
    pub geometry: Option<Geometry>,
}

#[derive(Clone)]
pub struct OutputManager {
    shm: Option<WlShm>,
    manager: Option<ZwlrScreencopyManagerV1>,
    pub outputs: Vec<(WlOutput, Output)>,
    intialized_outputs: u32,
    connection: Connection,
}

impl OutputManager {
    /// setup a new output manager which can be used to capture one or more frames of outputs or of selected regions
    pub fn new(connection: &Connection) -> Result<Self, Error> {
        let display = connection.display();

        let mut event_queue = connection.new_event_queue();
        let handle = event_queue.handle();

        let mut manager =
            Self { shm: None, manager: None, outputs: Vec::new(), intialized_outputs: 0, connection: connection.clone() };

        display.get_registry(&handle, ());

        event_queue.roundtrip(&mut manager).map_err(|err| Error::WaylandDispatch(err))?;

        if let None = manager.manager {
            Err(Error::ProtocolNotAvailable(std::any::type_name::<ZwlrScreencopyManagerV1>()))?
        }
        if let None = manager.shm {
            Err(Error::ProtocolNotAvailable(std::any::type_name::<WlShm>()))?
        }

        event_queue.roundtrip(&mut manager).map_err(|err| Error::WaylandDispatch(err))?;

        Ok(manager)
    }

    /// capture a single frame buffer of an output
    pub fn capture_output(&mut self, output: &WlOutput) -> Result<Buffer, Error> {
        let Some(zwlr_manager) = &self.manager else {
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
        let Some(zwlr_manager) = &self.manager else {
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
                    state.manager = Some(manager);
                }
                "wl_output" => {
                    let output: WlOutput = registry.bind(name, version, handle, ());
                    state.outputs.push((output, Output::default()));
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
        _proxy: &wl_output::WlOutput,
        event: <wl_output::WlOutput as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        let (_, output) = &mut state.outputs[state.intialized_outputs as usize];

        match event {
            wl_output::Event::Geometry { x, y, physical_width, physical_height, subpixel, make, model, transform } => {
                let geometry = Geometry {
                    x,
                    y,
                    physical_width,
                    physical_height,
                    make,
                    model,
                    subpixel: subpixel.into_result().expect("should be valid subpixel"),
                    transform: transform.into_result().expect("should be valid transform"),
                };
                output.geometry = Some(geometry);
            }
            wl_output::Event::Mode { flags, width, height, refresh } => {
                let mode = OutputMode { mode: flags.into_result().expect("should be valid mode"), width, height, refresh };
                output.mode = Some(mode)
            }
            wl_output::Event::Scale { factor } => output.scale = Some(factor),
            wl_output::Event::Name { name } => output.name = Some(name),
            wl_output::Event::Description { description } => output.description = Some(description),
            wl_output::Event::Done => state.intialized_outputs += 1,
            _ => {}
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
        let Some(data) = data.upgrade() else {
            log::debug!(
                "dispatcher for ZwlrScreencopyFrameV1 was called with event {event:?} but frame was already dropped"
            );
            return;
        };
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
