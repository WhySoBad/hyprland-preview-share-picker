use buffer::Buffer;
use protocols::hyprland_toplevel_export_v1::{
    hyprland_toplevel_export_frame_v1::{self, HyprlandToplevelExportFrameV1},
    hyprland_toplevel_export_manager_v1::HyprlandToplevelExportManagerV1,
};

use wayland_client::{
    delegate_noop, protocol::{
        wl_buffer::{self},
        wl_registry, wl_shm::{self}, wl_shm_pool,
    }, Connection, Dispatch
};

mod protocols;
mod buffer;

#[derive(Default)]
struct AppData {
    shm: Option<wl_shm::WlShm>,
    manager: Option<HyprlandToplevelExportManagerV1>,
    buffer: Option<Buffer>,
    ready: bool,
    failed: bool,
    buffer_done: bool
}

impl Dispatch<wl_registry::WlRegistry, ()> for AppData {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                match interface.as_str() {
                    "wl_shm" => {
                        let shm: wl_shm::WlShm = registry.bind(name, version, qhandle, ());
                        state.shm = Some(shm);
                    }
                    "hyprland_toplevel_export_manager_v1" => {
                        let manager: HyprlandToplevelExportManagerV1 = registry.bind(name, version, qhandle, ());
                        state.manager = Some(manager);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<HyprlandToplevelExportFrameV1, ()> for AppData {
    fn event(
        state: &mut Self,
        _proxy: &HyprlandToplevelExportFrameV1,
        event: <HyprlandToplevelExportFrameV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            hyprland_toplevel_export_frame_v1::Event::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                println!("received buffer requirements: {format:?} width={width} height={height} stride={stride}");
                let format = format.into_result().expect("should be valid format");
                if let Some(shm) = &state.shm {
                    match Buffer::new(shm, width, height, stride, format, qhandle) {
                        Ok(buffer) => state.buffer = Some(buffer),
                        Err(err) => println!("error whilst creating buffer: {err}")
                    }
                } else {
                    todo!("throw error when this is called without having shm");
                }
            }
            hyprland_toplevel_export_frame_v1::Event::Damage {
                x,
                y,
                width,
                height,
            } => {
                println!("received damage x={x} y={y} width={width} height={height}");
            }
            hyprland_toplevel_export_frame_v1::Event::Flags { flags } => {
                println!("received flags {flags:?}");
                todo!("parse flags")
            }
            #[allow(unused)]
            hyprland_toplevel_export_frame_v1::Event::Ready {
                tv_sec_hi,
                tv_sec_lo,
                tv_nsec,
            } => {
                println!("received frame ready event");
                state.ready = true;
            }
            hyprland_toplevel_export_frame_v1::Event::Failed => {
                println!("received failed event");
                state.failed = true;
            }
            hyprland_toplevel_export_frame_v1::Event::LinuxDmabuf {
                format,
                width,
                height,
            } => {
                println!("received dma buf data format={format} width={width} height={height}");
            }
            hyprland_toplevel_export_frame_v1::Event::BufferDone => {
                println!("received buffer done event");
                state.buffer_done = true;
            }
        }
    }
}

delegate_noop!(AppData: ignore wl_shm::WlShm);
delegate_noop!(AppData: ignore wl_shm_pool::WlShmPool);
delegate_noop!(AppData: ignore wl_buffer::WlBuffer);
delegate_noop!(AppData: ignore HyprlandToplevelExportManagerV1);

const WINDOW_HANDLE: u64 = 0x59d5f3cec450;

fn main() {
    let con = Connection::connect_to_env().expect("should connect to environment");
    let display = con.display();

    let mut event_queue = con.new_event_queue();
    let qhandle = event_queue.handle();

    display.get_registry(&qhandle, ());

    let mut app_data = AppData::default();

    event_queue
        .roundtrip(&mut app_data)
        .expect("should roundtrip");

    let Some((manager, _)) = app_data
        .manager.clone()
        .zip(app_data.shm.clone())
    else {
        println!("manager or shm not initialized correctly");
        return;
    };

    let frame = manager.capture_toplevel(0, WINDOW_HANDLE as u32, &qhandle, ());

    println!("waiting for buffer...");
    while !app_data.buffer_done && !app_data.failed {
        event_queue.blocking_dispatch(&mut app_data).expect("should dispatch");
    }

    if let Some(Buffer { buffer, .. }) = &app_data.buffer {
        println!("found buffer");
        frame.copy(buffer, 0);
        println!("sent copy request");
    };

    while !app_data.ready && !app_data.failed {
        event_queue.blocking_dispatch(&mut app_data).expect("should dispatch");
    }

    if app_data.ready {
        println!("frame is ready");
        let frame_buffer = app_data.buffer.expect("should have frame information");
        let img = image::RgbaImage::from_vec(
            frame_buffer.width,
            frame_buffer.height,
            frame_buffer.get_bytes().expect("should get bytes")
        ).expect("should create rgba image");
        img.save("out.png").expect("should save png");
        frame_buffer.destroy().expect("should destroy buffer");
    } else {
        println!("copy failed")
    }
}