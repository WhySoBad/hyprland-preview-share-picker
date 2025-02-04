use std::sync::Arc;

use wayland_client::{delegate_noop, protocol::{wl_buffer::WlBuffer, wl_output::{self, Mode, Subpixel, Transform, WlOutput}, wl_registry, wl_shm::WlShm, wl_shm_pool::WlShmPool}, Connection, Dispatch, EventQueue};
use wayland_protocols_wlr::screencopy::v1::client::{zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1}, zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1};

use crate::{buffer::Buffer, frame::FrameStatus};

#[derive(Debug, Clone)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub physical_width: i32,
    pub physical_height: i32,
    pub subpixel: Subpixel,
    pub make: String,
    pub model: String,
    pub transform: Transform
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
    pub geometry: Option<Geometry>
}

pub struct OutputManager {
    shm: Option<WlShm>,
    manager: Option<ZwlrScreencopyManagerV1>,
    pub outputs: Vec<(WlOutput, Output)>,
    intialized_outputs: u32,
    status: FrameStatus,
    connection: Connection
}

impl OutputManager {
    pub fn new(connection: &Connection) -> Result<Self, Box<dyn std::error::Error>> {
        let display = connection.display();

        let mut event_queue = connection.new_event_queue();
        let handle = event_queue.handle();

        let mut manager = Self {
            shm: None,
            manager: None,
            outputs: Vec::new(),
            intialized_outputs: 0,
            status: FrameStatus::Inactive,
            connection: connection.clone()
        };

        display.get_registry(&handle, ());

        event_queue.roundtrip(&mut manager)?;

        if let None = manager.manager {
            return Err(Box::from("zwlr screencopy manager v1 is not available"))
        }
        if let None = manager.shm {
            return Err(Box::from("wl shm is not available"))
        }

        event_queue.roundtrip(&mut manager)?;

        Ok(manager)
    }

    pub fn capture_output(&mut self, output: &WlOutput) -> Result<Arc<Buffer>, Box<dyn std::error::Error>> {
        let &FrameStatus::Inactive = &self.status else {
            return Err(Box::from("output manager is not in inactive status"))
        };

        let Some(zwlr_manager) = &self.manager else {
            return Err(Box::from("zwlr screencopy manager is not available"));
        };

        self.status = FrameStatus::Active;

        let mut event_queue = self.connection.new_event_queue();
        let handle = event_queue.handle();
        let zwlr_frame = zwlr_manager.capture_output(0, output, &handle, ());
        self.finish_capture(zwlr_frame, &mut event_queue)
    }

    pub fn capture_output_region(&mut self, output: &WlOutput, x: i32, y: i32, width: i32, height: i32) -> Result<Arc<Buffer>, Box<dyn std::error::Error>> {
        let &FrameStatus::Inactive = &self.status else {
            return Err(Box::from("output manager is not in inactive status"))
        };

        let Some(zwlr_manager) = &self.manager else {
            return Err(Box::from("zwlr screencopy manager is not available"));
        };

        self.status = FrameStatus::Active;

        let mut event_queue = self.connection.new_event_queue();
        let handle = event_queue.handle();
        let zwlr_frame = zwlr_manager.capture_output_region(0, output, x, y, width, height, &handle, ());
        self.finish_capture(zwlr_frame, &mut event_queue)
    }

    fn finish_capture(&mut self, zwlr_frame: ZwlrScreencopyFrameV1, event_queue: &mut EventQueue<OutputManager>) -> Result<Arc<Buffer>, Box<dyn std::error::Error>> {
        loop {
            if let Err(err) = event_queue.blocking_dispatch(self) {
                self.status = FrameStatus::Inactive;
                Err(err)?;
            }
            match &self.status {
                FrameStatus::FrameReady(buffer) => {
                    let buffer = buffer.clone();
                    zwlr_frame.destroy();
                    self.status = FrameStatus::Inactive;
                    return Ok(buffer);
                },
                FrameStatus::BufferDone(buffer) => {
                    zwlr_frame.copy(&buffer.buffer);
                    self.status = FrameStatus::FrameRequested(buffer.clone());
                },
                FrameStatus::Error(err) => {
                    let err = Box::from(format!("error during frame capture: {err}"));
                    self.status = FrameStatus::Inactive;
                    zwlr_frame.destroy();
                    return Err(err)
                }
                FrameStatus::Failed => {
                    self.status = FrameStatus::Inactive;
                    zwlr_frame.destroy();
                    return Err(Box::from("frame copy failed"))
                },
                _ => {}
            }
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
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                match interface.as_str() {
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
                }
            }
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
                    transform: transform.into_result().expect("should be valid transform")
                };
                output.geometry = Some(geometry);
            },
            wl_output::Event::Mode { flags, width, height, refresh } => {
                let mode = OutputMode {
                    mode: flags.into_result().expect("should be valid mode"),
                    width,
                    height,
                    refresh
                };
                output.mode = Some(mode)
            },
            wl_output::Event::Scale { factor } => output.scale = Some(factor),
            wl_output::Event::Name { name } => output.name = Some(name),
            wl_output::Event::Description { description } => output.description = Some(description),
            wl_output::Event::Done => state.intialized_outputs += 1,
            _ => {},
        }
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, ()> for OutputManager {
    fn event(
        state: &mut Self,
        _proxy: &ZwlrScreencopyFrameV1,
        event: <ZwlrScreencopyFrameV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer { format, width, height, stride } => {
                let Ok(format) = format.into_result() else {
                    state.status = FrameStatus::Error(Box::from("buffer format was not valid enum"));
                    return;
                };
                if let Some(shm) = &state.shm {
                    match Buffer::new(shm, width, height, stride, format, qhandle, ()) {
                        Ok(buffer) => {
                            state.status = FrameStatus::BufferDone(Arc::new(buffer))
                        },
                        Err(err) => {
                            state.status = FrameStatus::Error(Box::from(format!("unable to create buffer: {err}")))
                        }
                    }
                } else {
                    state.status = FrameStatus::Error(Box::from("buffer event is called without having shm"));
                }
            },
            zwlr_screencopy_frame_v1::Event::Flags { flags } => {},
            zwlr_screencopy_frame_v1::Event::Ready { .. } => {
                if let FrameStatus::FrameRequested(buffer) = &state.status {
                    state.status = FrameStatus::FrameReady(buffer.clone())
                } else {
                    state.status = FrameStatus::Error(Box::from("received frame ready without having requested a frame"))
                }
            },
            zwlr_screencopy_frame_v1::Event::Failed => state.status = FrameStatus::Failed,
            zwlr_screencopy_frame_v1::Event::Damage { .. } => {},
            zwlr_screencopy_frame_v1::Event::LinuxDmabuf { .. } => {},
            zwlr_screencopy_frame_v1::Event::BufferDone => {},
            _ => {},
        }
    }
}

delegate_noop!(OutputManager: ignore WlShm);
delegate_noop!(OutputManager: ignore WlShmPool);
delegate_noop!(OutputManager: ignore WlBuffer);
delegate_noop!(OutputManager: ignore ZwlrScreencopyManagerV1);