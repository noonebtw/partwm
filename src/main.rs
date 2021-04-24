use log::info;
use std::io::Result;

mod clients;
mod state;
mod util;
mod wm;
mod xlib;

#[allow(dead_code)]
unsafe extern "C" fn xlib_error_handler(
    _dpy: *mut x11::xlib::Display,
    ee: *mut x11::xlib::XErrorEvent,
) -> std::os::raw::c_int {
    let err = ee.as_ref().unwrap();

    if err.error_code == x11::xlib::BadWindow
        || err.error_code == x11::xlib::BadDrawable
        || err.error_code == x11::xlib::BadAccess
        || err.error_code == x11::xlib::BadMatch
    {
        0
    } else {
        eprintln!(
            "wm: fatal error:\nrequest_code: {}\nerror_code: {}",
            err.request_code, err.error_code
        );
        std::process::exit(1);
    }
}

fn main() -> Result<()> {
    simple_logger::SimpleLogger::new().init().unwrap();
    info!("Hello, World!");

    //wm::WMState::init().run();
    state::WindowManager::new().run();
}
