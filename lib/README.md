# hyprland-preview-share-picker-lib

This project contains the code for everything related to communication with the hyprland server.

## Installation

You can add the library to your project using the following line in the `Cargo.toml` file:
```toml
[dependencies]
hyprland-preview-share-picker-lib = { git = "https://github.com/WhySoBad/hyprland-preview-share-picker" }
```

If you want to use monitor transformations it's recommended to enable the `hyprland-rs` feature flag which is disabled by default.

## Usage

Using this library it's very easy to capture frames using the `hyprland-toplevel-export-v1` and `wlr-screencopy-unstable-v1` protocols.

```rust
use wayland_client::Connection;
use hyprland_preview_share_picker_lib::*;

// window handle as returned by `hyprctl clients -j` in the `address` field
const WINDOW_HANDLE: u64 = 0x5713073a6a20;

fn main() {
    let connection = Connection::connect_to_env().unwrap();

    // initialize new frame manager which can be used to capture multiple frames
    let mut frame_manager = FrameManager::new(&connection).unwrap();
    let buffer = frame_manager.capture_frame(WINDOW_HANDLE).unwrap();
    let image = Image::new(buffer).unwrap();
    // do something with the image

    // initialize new output manager which can be used to capture multiple frames
    let mut output_manager = OutputManager::new(&connection).unwrap();
    // take first output and capture it
    let (wl_output, output) = output_manager.outputs.first().unwrap().clone();
    let buffer = output_manager.capture_output(&wl_output).unwrap();
    let image = Image::new(buffer).unwrap();
    // do something with the image
}
```