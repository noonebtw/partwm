use self::window_event::KeyBind;

pub mod keycodes;
pub mod window_event;

pub trait WindowServerBackend<Window = u64> {
    fn next_event(&self) -> window_event::WindowEvent;
    fn add_keybind(&mut self, keybind: KeyBind, window: Some<Window>);
    fn remove_keybind(&mut self, keybind: KeyBind, window: Some<Window>);
    fn add_mousebind(&mut self, keybind: KeyBind, window: Some<Window>);
    fn remove_mousebind(&mut self, keybind: KeyBind, window: Some<Window>);
    fn focus_window(&self, window: Window);
    fn unfocus_window(&self, window: Window);
    fn move_window(&self, window: Window, pos: i32);
    fn resize_window(&self, window: Window, pos: i32);
    fn hide_window(&self, window: Window);
    fn screen_size(&self) -> (i32, i32);
    fn kill_window(&self, window: Window);
}

pub mod xlib {
    use std::ffi::CString;

    use x11::xlib::{Atom, Window, XInternAtom};

    #[derive(Clone)]
    pub struct Display(Rc<*mut xlib::Display>);

    impl Deref for Display {}

    impl Display {
        pub fn new(display: *mut x11::xlib::Display) -> Self {
            Self {
                0: Rc::new(display),
            }
        }

        pub fn get(&self) -> *mut x11::xlib::Display {
            *self.0
        }
    }

    pub struct XLib {
        display: Display,
        root: Window,
        screen: i32,
        atoms: XLibAtoms,
        keybinds: Vec<()>,
    }

    struct XLibAtoms {
        protocols: Atom,
        delete_window: Atom,
        active_window: Atom,
        take_focus: Atom,
    }

    impl XLibAtoms {
        fn init(display: Display) -> Self {
            unsafe {
                Self {
                    protocols: {
                        let name = CString::new("WM_PROTOCOLS").unwrap();
                        XInternAtom(display.get(), name.as_c_str().as_ptr(), 0)
                    },
                    delete_window: {
                        let name = CString::new("WM_DELETE_WINDOW").unwrap();
                        XInternAtom(display.get(), name.as_c_str().as_ptr(), 0)
                    },
                    active_window: {
                        let name = CString::new("WM_ACTIVE_WINDOW").unwrap();
                        XInternAtom(display.get(), name.as_c_str().as_ptr(), 0)
                    },
                    take_focus: {
                        let name = CString::new("WM_TAKE_FOCUS").unwrap();
                        XInternAtom(display.get(), name.as_c_str().as_ptr(), 0)
                    },
                }
            }
        }
    }

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
            error!(
                "wm: fatal error:\nrequest_code: {}\nerror_code: {}",
                err.request_code, err.error_code
            );
            std::process::exit(1);
        }
    }
}
