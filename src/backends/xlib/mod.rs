use log::{error, warn};
use std::{
    convert::{TryFrom, TryInto},
    ffi::CString,
    rc::Rc,
};
use thiserror::Error;

use x11::xlib::{self, Atom, Window, XEvent, XInternAtom};

use self::keysym::xev_to_mouse_button;

use super::{
    keycodes::{MouseButton, VirtualKeyCode},
    window_event::{
        ButtonEvent, ConfigureEvent, DestroyEvent, EnterEvent, KeyState,
        MapEvent, ModifierState, UnmapEvent, WindowEvent,
    },
    WindowServerBackend,
};

pub mod keysym;

#[derive(Clone)]
pub struct Display(Rc<*mut x11::xlib::Display>);

#[derive(Debug, Error)]
pub enum XlibError {
    #[error("BadAccess")]
    BadAccess,
    #[error("BadAlloc")]
    BadAlloc,
    #[error("BadAtom")]
    BadAtom,
    #[error("BadColor")]
    BadColor,
    #[error("BadCursor")]
    BadCursor,
    #[error("BadDrawable")]
    BadDrawable,
    #[error("BadFont")]
    BadFont,
    #[error("BadGC")]
    BadGC,
    #[error("BadIDChoice")]
    BadIDChoice,
    #[error("BadImplementation")]
    BadImplementation,
    #[error("BadLength")]
    BadLength,
    #[error("BadMatch")]
    BadMatch,
    #[error("BadName")]
    BadName,
    #[error("BadPixmap")]
    BadPixmap,
    #[error("BadRequest")]
    BadRequest,
    #[error("BadValue")]
    BadValue,
    #[error("BadWindow")]
    BadWindow,
    #[error("Invalid XError: {0}")]
    InvalidError(u8),
}

impl From<u8> for XlibError {
    fn from(value: u8) -> Self {
        match value {
            xlib::BadAccess => XlibError::BadAccess,
            xlib::BadAlloc => XlibError::BadAlloc,
            xlib::BadAtom => XlibError::BadAtom,
            xlib::BadColor => XlibError::BadColor,
            xlib::BadCursor => XlibError::BadCursor,
            xlib::BadDrawable => XlibError::BadDrawable,
            xlib::BadFont => XlibError::BadFont,
            xlib::BadGC => XlibError::BadGC,
            xlib::BadIDChoice => XlibError::BadIDChoice,
            xlib::BadImplementation => XlibError::BadImplementation,
            xlib::BadLength => XlibError::BadLength,
            xlib::BadMatch => XlibError::BadMatch,
            xlib::BadName => XlibError::BadName,
            xlib::BadPixmap => XlibError::BadPixmap,
            xlib::BadRequest => XlibError::BadRequest,
            xlib::BadValue => XlibError::BadValue,
            xlib::BadWindow => XlibError::BadWindow,
            any => XlibError::InvalidError(any),
        }
    }
}

// impl Into<i32> for XlibError {
//     fn into(self) -> i32 {
//         match self {
//             XlibError::BadAccess => xlib::BadAccess.into(),
//             XlibError::BadAlloc => xlib::BadAlloc.into(),
//             XlibError::BadAtom => xlib::BadAtom.into(),
//             XlibError::BadColor => xlib::BadColor.into(),
//             XlibError::BadCursor => xlib::BadCursor.into(),
//             XlibError::BadDrawable => xlib::BadDrawable.into(),
//             XlibError::BadFont => xlib::BadFont.into(),
//             XlibError::BadGC => xlib::BadGC.into(),
//             XlibError::BadIDChoice => xlib::BadIDChoice.into(),
//             XlibError::BadImplementation => xlib::BadImplementation.into(),
//             XlibError::BadLength => xlib::BadLength.into(),
//             XlibError::BadMatch => xlib::BadMatch.into(),
//             XlibError::BadName => xlib::BadName.into(),
//             XlibError::BadPixmap => xlib::BadPixmap.into(),
//             XlibError::BadRequest => xlib::BadRequest.into(),
//             XlibError::BadValue => xlib::BadValue.into(),
//             XlibError::BadWindow => xlib::BadWindow.into(),
//             XlibError::InvalidError(err) => err,
//         }
//     }
// }

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

impl XLib {
    fn dpy(&self) -> *mut xlib::Display {
        self.display.get()
    }

    fn next_xevent(&self) -> XEvent {
        unsafe {
            let mut event =
                std::mem::MaybeUninit::<xlib::XEvent>::zeroed().assume_init();
            xlib::XNextEvent(self.dpy(), &mut event);

            event
        }
    }
}

impl TryFrom<XEvent> for WindowEvent<xlib::Window> {
    type Error = crate::error::Error;

    fn try_from(event: XEvent) -> Result<Self, Self::Error> {
        match event.get_type() {
            xlib::MapRequest => {
                let ev = unsafe { &event.map_request };
                Ok(Self::MapRequestEvent(MapEvent { window: ev.window }))
            }
            xlib::UnmapNotify => {
                let ev = unsafe { &event.unmap };
                Ok(Self::UnmapEvent(UnmapEvent { window: ev.window }))
            }
            xlib::ConfigureRequest => {
                let ev = unsafe { &event.configure_request };
                Ok(Self::ConfigureEvent(ConfigureEvent {
                    window: ev.window,
                    position: [ev.x, ev.y],
                    size: [ev.width, ev.height],
                }))
            }
            xlib::EnterNotify => {
                let ev = unsafe { &event.crossing };
                Ok(Self::EnterEvent(EnterEvent { window: ev.window }))
            }
            xlib::DestroyNotify => {
                let ev = unsafe { &event.destroy_window };
                Ok(Self::DestroyEvent(DestroyEvent { window: ev.window }))
            }
            xlib::ButtonPress | xlib::ButtonRelease => {
                let ev = unsafe { &event.button };
                let keycode = xev_to_mouse_button(ev).unwrap();
                let state = if ev.state as i32 == xlib::ButtonPress {
                    KeyState::Pressed
                } else {
                    KeyState::Released
                };

                let modifierstate = ModifierState::new();

                Ok(Self::ButtonEvent(ButtonEvent::new(
                    ev.window,
                    state,
                    keycode,
                    modifierstate,
                )))
            }
            _ => Err(Self::Error::UnknownEvent),
        }
    }
}

impl WindowServerBackend for XLib {
    type Window = xlib::Window;

    fn next_event(&self) -> super::window_event::WindowEvent<Self::Window> {
        self.next_xevent().try_into().unwrap()
    }

    fn add_keybind(
        &mut self,
        keybind: super::window_event::KeyBind,
        window: Option<Self::Window>,
    ) {
        todo!()
    }

    fn remove_keybind(
        &mut self,
        keybind: super::window_event::KeyBind,
        window: Option<Self::Window>,
    ) {
        todo!()
    }

    fn add_mousebind(
        &mut self,
        keybind: super::window_event::KeyBind,
        window: Option<Self::Window>,
    ) {
        todo!()
    }

    fn remove_mousebind(
        &mut self,
        keybind: super::window_event::KeyBind,
        window: Option<Self::Window>,
    ) {
        todo!()
    }

    fn focus_window(&self, window: Self::Window) {
        todo!()
    }

    fn unfocus_window(&self, window: Self::Window) {
        todo!()
    }

    fn move_window(&self, window: Self::Window, pos: i32) {
        todo!()
    }

    fn resize_window(&self, window: Self::Window, pos: i32) {
        todo!()
    }

    fn hide_window(&self, window: Self::Window) {
        todo!()
    }

    fn screen_size(&self) -> (i32, i32) {
        todo!()
    }

    fn kill_window(&self, window: Self::Window) {
        todo!()
    }
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
    let err_event = ee.as_ref().unwrap();
    let err = XlibError::from(err_event.error_code);

    match err {
        err @ XlibError::BadAccess
        | err @ XlibError::BadMatch
        | err @ XlibError::BadWindow
        | err @ XlibError::BadDrawable => {
            warn!("{:?}", err);
            0
        }
        _ => {
            error!(
                "wm: fatal error:\nrequest_code: {}\nerror_code: {}",
                err_event.request_code, err_event.error_code
            );
            std::process::exit(1)
        }
    }
}
