# hyprland-share-picker

<div align="center" justify="center">
  <img width="90%" src="https://github.com/user-attachments/assets/0172f531-08b5-48c7-b167-32c6ce6535e8" />
</div>
<div align="center">
    <i>the screenshot was made using a custom stylesheet, the widgets use the gtk theme per default <sup><a href="#customization">[1]</a></sup></i>
</div>

## Configuration

The default configuration path is `$XDG_CONFIG_DIR/hyprland-share-picker/config.yaml` with a fallback to `~/.config/hyprland-share-picker/config.yaml`.
The configuration path can be overwritten using the `-c/--config` cli argument.

Below is a configuration file with all fields and their default values:

```yaml
# paths to stylesheets on the filesystem which should be applied to the application
#
# relative paths are resolved relative to the location of the config file
stylesheets: []

window:
  # height of the application window
  height: 500
  # width of the application window
  width: 1000

image:
  # size to which the images should be internally resized to reduce the memory footprint
  resize_size: 200
  # target height of the image widget
  widget_size: 150

classes:
  # css classname of the window
  window: window
  # css classname of the card containing an image and a label
  image_card: card
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

windows:
  # minimum amount of image cards per row on the windows page
  min_per_row: 3
  # maximum amount of image cards per row on the windows page
  max_per_row: 999

outputs:
  # minimum amount of image cards per row on the outputs page
  min_per_row: 2
  # maximum amount of image cards per row on the outputs page
  max_per_row: 2

region:
  # command to run for region selection
  # the output needs to be in the <output>@<x>,<y>,<w>,<h> (e.g. DP-3@2789,436,756,576) format
  command: slurp -f '%o@%x,%y,%w,%h'
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

### Custom frontend

If you prefer to have a frontend in the ui toolkit of your choice or you dislike the layout of this frontend, it should be pretty straightforward to
create your own frontend in rust. All of the wayland logic is located in the `hyprland-share-picker-protocols` project. By adding this as git dependency
to your project, most of the application logic should be taken care of.