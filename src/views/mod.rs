use gtk4::{Label, ScrolledWindow};

pub mod outputs;
pub mod region;
pub mod windows;

pub trait View {
    fn build(&self) -> ScrolledWindow;
    fn label(&self) -> Label;
}
