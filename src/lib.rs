pub mod backends;
pub mod clients;
pub mod state;
pub mod util;

pub mod error {
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum Error {
        #[error("placeholder error for Result<T> as Option<T>")]
        NonError,
        #[error("Unknown Event")]
        UnknownEvent,
        #[error("Unhandled VirtualKeyCode")]
        UnhandledVirtualKeyCode,
        #[error(transparent)]
        IoError(#[from] std::io::Error),
        #[error(transparent)]
        FmtError(#[from] std::fmt::Error),
        #[error(transparent)]
        XlibError(#[from] crate::backends::xlib::XlibError),
    }
}
