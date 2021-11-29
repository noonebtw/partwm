use std::mem::MaybeUninit;

use x11::{xft, xlib};

use super::Display;

pub struct XftColor {
    inner: xft::XftColor,
}

impl XftColor {
    pub fn pixel(&self) -> u64 {
        self.inner.pixel
    }

    #[allow(dead_code)]
    pub fn color(&self) -> x11::xrender::XRenderColor {
        self.inner.color
    }

    pub fn new(
        dpy: Display,
        screen: i32,
        mut color_name: String,
    ) -> Result<Self, std::io::Error> {
        color_name.push('\0');
        let mut color = MaybeUninit::<xft::XftColor>::zeroed();

        unsafe {
            xft::XftColorAllocName(
                dpy.get(),
                xlib::XDefaultVisual(dpy.get(), screen),
                xlib::XDefaultColormap(dpy.get(), screen),
                color_name.as_ptr() as *mut _,
                color.as_mut_ptr(),
            ) != 0
        }
        .then(|| Self {
            inner: unsafe { color.assume_init() },
        })
        .ok_or(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Unable to allocate color.",
        ))
    }
}
