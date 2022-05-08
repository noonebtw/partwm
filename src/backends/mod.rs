pub mod keycodes;
pub mod traits;
pub mod window_event;
pub mod xlib;

pub use traits::*;

pub mod structs {

    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
    pub enum WindowType {
        Splash,
        Dialog,
        Normal,
        Utility,
        Menu,
        Toolbar,
        Dock,
        Desktop,
    }
}
