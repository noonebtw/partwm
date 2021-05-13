use std::ptr::null;

use x11::xlib::{Window, XRootWindow};

// xlib backend

pub struct XLib {
    display: *mut x11::xlib::Display,
    screen: i32,
}

impl Drop for XLib {
    fn drop(&mut self) {
        unsafe {
            x11::xlib::XCloseDisplay(self.display);
        }
    }
}

impl XLib {
    pub fn new() -> Self {
        let (display, screen) = unsafe {
            let display = x11::xlib::XOpenDisplay(null());
            let screen = x11::xlib::XDefaultScreen(display);

            (display, screen)
        };
        Self { display, screen }
    }

    fn root_window(&self) -> Window {
        unsafe { XRootWindow(self.display, self.screen) }
    }
}
