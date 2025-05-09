# hyprland-preview-share-picker

<div align="center" justify="center">
  <img width="90%" src="https://github.com/user-attachments/assets/0172f531-08b5-48c7-b167-32c6ce6535e8" />
</div>
<div align="center">
    <i>the screenshot was made using a custom stylesheet, the widgets use the gtk theme per default <sup><a href="#customization">[1]</a></sup></i>
</div>

## Installation

### Using pacman

On Arch-based systems, the easiest and recommended way to install the share picker is using the offical [hyprland-preview-share-picker-git](https://aur.archlinux.org/packages/hyprland-preview-share-picker-git) AUR-package.

Alternatively, the following `PKGBUILD` can be used to build and install the package locally. Simply copy the `PKGBUILD` source to an empty directory on your system and install the package and all it's dependencies using `makepkg -si`

<details>
<summary><b>PKGBUILD source</b></summary>

```bash
pkgname="hyprland-preview-share-picker-git"
pkgver=v0.1.0
pkgrel=1
pkgdesc="An alternative share picker for hyprland with window and monitor previews"
arch=(x86_64)
url="https://github.com/WhySoBad/hyprland-preview-share-picker"
license=(MIT)
depends=('gtk4' 'gtk4-layer-shell' 'xdg-desktop-portal-hyprland' 'hyprland')
makedepends=(cargo-nightly)
optdepends=(
  'slurp: default tool for selecting share regions'
)
source=("$pkgname::git+https://github.com/WhySoBad/hyprland-preview-share-picker")
md5sums=('SKIP')

pkgver() {
    cd "$pkgname"
    git describe --long --abbrev=7 --tags | sed -E 's/^[^0-9]*//;s/([^-]*-g)/r\1/;s/-/./g'
}

prepare() {
    cd "$pkgname"
    git submodule init
    git config submodule.subprojects/lib.url "$srcdir/lib"
    git -c protocol.file.allow=always submodule update

    export RUSTUP_TOOLCHAIN=nightly
    cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
    cd "$pkgname"

    export RUSTUP_TOOLCHAIN=nightly
    export CARGO_TARGET_DIR=target

    cargo build --frozen --release

    ./target/release/hyprland-preview-share-picker schema > schema.json
}

package() {
    cd "$pkgname"

    install -Dm0755 -T "target/release/hyprland-preview-share-picker" "$pkgdir/usr/bin/hyprland-preview-share-picker"

    install -dm0755 "$pkgdir/usr/share/hyprland-preview-share-picker"
    install -Dm0644 "schema.json" "$pkgdir/usr/share/hyprland-preview-share-picker"
}
```

</details>

### Building yourself

The following dependencies are needed:
- gtk4
- gtk4-layer-shell
- xdg-desktop-portal-hyprland (xdg-desktop-portal-hyprland-git)
- hyprland (hyprland-git)

> Depending on your distribution the names may differ, the above names are for the Arch and AUR packages

The project is built using the rust nightly toolchain, make sure you're using it for this build.

```bash
# clone the repository with it's submodules
git clone --recursive https://github.com/WhySoBad/hyprland-preview-share-picker

cd ./hyprland-preview-share-picker

# build the optimized release binary
cargo build --release
```
The built binary is now available in the `target/release/hyprland-preview-share-picker` directory. If you want to install it directly using
cargo you can use the following command. However, make sure the cargo binary directory is added to your path:

```bash
# install the package into your cargo binary directory
cargo install --path .
```

## Usage

Once installed, you need to change the [xdg-desktop-portal-hyprland screencopy configuration](https://wiki.hyprland.org/Hypr-Ecosystem/xdg-desktop-portal-hyprland/#category-screencopy) to use the `hyprland-preview-share-picker` binary as picker:

```ini
# ~/.config/hypr/xdph.conf
screencopy {
  custom_picker_binary = hyprland-preview-share-picker
}
```

After changing the config the portal needs to be restarted.

## Configuration

The default configuration path is `$XDG_CONFIG_DIR/hyprland-preview-share-picker/config.yaml` with a fallback to `~/.config/hyprland-preview-share-picker/config.yaml`.
The configuration path can be overwritten using the `-c/--config` cli argument.

Below is a configuration file with all fields and their default values:

```yaml
# paths to stylesheets on the filesystem which should be applied to the application
#
# relative paths are resolved relative to the location of the config file
stylesheets: []
# default page selected when the picker is opened
default_page: windows

window:
  # height of the application window
  height: 500
  # width of the application window
  width: 1000

image:
  # size to which the images should be internally resized to reduce the memory footprint
  resize_size: 200
  # target size of the longer side of the image widget
  widget_size: 150

classes:
  # css classname of the window
  window: window
  # css classname of the card containing an image and a label
  image_card: card
  # css classname of the card containing an image and a label when the image is still being loaded
  image_card_loading: card-loading
  # css classname of the image inside the card
  image: image
  # css classname of the label inside the card
  image_label: image-label
  # css classname of the notebook containing all pages
  notebook: notebook
  # css classname of a label of the notebook
  tab_label: tab-label
  # css classname of a notebook page (e.g. windows container)
  notebook_page: page
  # css classname of the region selection button
  region_button: region-button
  # css classname of the button containing the session restore checkbox and label
  restore_button: restore-button

windows:
  # minimum amount of image cards per row on the windows page
  min_per_row: 3
  # maximum amount of image cards per row on the windows page
  max_per_row: 999
  # number of clicks needed to select a window
  clicks: 2
  # spacing in pixels between the window cards
  spacing: 12

outputs:
  # number of clicks needed to select an output
  clicks: 2
  # spacing in pixels between the outputs in the layout
  # note: the spacing is applied from both sides (the gap is `spacing * 2`)
  spacing: 6
  # show the label with the output name
  show_label: false
  # size the output cards respectively to their scaling
  respect_output_scaling: true

region:
  # command to run for region selection
  # the output needs to be in the <output>@<x>,<y>,<w>,<h> (e.g. DP-3@2789,436,756,576) format
  command: slurp -f '%o@%x,%y,%w,%h'

# hide the token restore checkbox and use the default value instead
hide_token_restore: false
# enable debug logs by default
debug: false
```

<details>
<summary><b>Schema for config file</b></summary>

A JSON schema for the configuration file can be generated using the `schema` subcommand.
For editor support you need to configure your YAML language server to apply this schema to the config file.

</details>

## Customization

The widgets use their default gtk style out of the box. Using the `stylesheets` config field an array of paths to CSS/SCSS stylesheets
can be provided which then are applied to the application.

It's possible to override most of the CSS classnames of the widgets used with the `classes` config field.

<details>

<summary><b>Example stylesheet from the screenshot</b></summary>

```css
* {
  all: unset;
  font-family: JetBrains Mono NF;
  color: #ECF2F8;
  font-weight: bold;
  font-size: 16px;
}

.window {
  border-radius: 5px;
  background-color: #0D1117;
  border: solid 2px #21262D;
  margin: 2px;
}

tabs {
    padding: 0.5rem 1rem;
}

tabs > tab {
    margin-right: 1rem;
}

.tab-label {
    color: #89929B;
    transition: all 0.2s ease;
}

tabs > tab:checked > .tab-label, tabs > tab:active > .tab-label {
    text-decoration: underline currentColor;
    color: #ECF2F8;
}

tabs > tab:focus > .tab-label {
    color: #ECF2F8;
}

.page {
    padding: 1rem;
}

.image-label {
    font-size: 12px;
    padding: 0.25rem;
}

flowboxchild > .card, button > .card {
    transition: all 0.2s ease;
    border: solid 2px transparent;
    border-color: #161B22;
    border-radius: 5px;
    background-color: #161B22;
    padding: 5px;
}

flowboxchild:active > .card, flowboxchild:selected > .card, button:active > .card, button:selected > .card, button:focus > .card {
    border: solid 2px #61AFEF;
}

.image {
    border-radius: 5px;
}

.region-button {
    padding: 0.5rem 1rem;
    border-radius: 5px;
    background-color: #61AFEF;
    color: #161B22;
    transition: all 0.2s ease;
}

.region-button:not(:disabled):hover, .region-button:not(:disabled):focus {
    background-color: #2472C8;
}

.region-button:disabled {
    background-color: #89929B;
    color: #21262D;
}
```

</details>

### Custom frontend

If you prefer to have a frontend in the ui toolkit of your choice or you dislike the layout of this frontend, it should be pretty straightforward to
create your own frontend in rust. All of the toolkit independent logic (mostly wayland logic) is located in the `lib` subproject. By adding this as git dependency
to your project, most of the application logic should be taken care of.