use wayland_backend::protocol::WEnumError;
use wayland_client::DispatchError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("the frame capture failed")]
    Failed,
    #[error("no protocol object of type {0} was registered")]
    ProtocolNotAvailable(&'static str),
    #[error("unable to parse protocol enum: {0}")]
    ProtocolInvalidEnum(WEnumError),
    #[error("error whilst dispatching: {0}")]
    WaylandDispatch(DispatchError),
    #[error("tried to create buffer without having shm registered")]
    NoShm,
    #[error("unable to read buffer: {0}")]
    BufferRead(std::io::Error),
    #[error("unable to create buffer: {0}")]
    BufferCreate(Box<dyn std::error::Error + Sync + Send>),
}
