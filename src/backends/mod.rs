use self::window_event::KeyBind;

pub mod keycodes;
pub mod window_event;
pub mod xlib;

pub trait WindowServerBackend<Window = u64> {
    fn next_event(&self) -> window_event::WindowEvent;
    fn add_keybind(&mut self, keybind: KeyBind, window: Option<Window>);
    fn remove_keybind(&mut self, keybind: KeyBind, window: Option<Window>);
    fn add_mousebind(&mut self, keybind: KeyBind, window: Option<Window>);
    fn remove_mousebind(&mut self, keybind: KeyBind, window: Option<Window>);
    fn focus_window(&self, window: Window);
    fn unfocus_window(&self, window: Window);
    fn move_window(&self, window: Window, pos: i32);
    fn resize_window(&self, window: Window, pos: i32);
    fn hide_window(&self, window: Window);
    fn screen_size(&self) -> (i32, i32);
    fn kill_window(&self, window: Window);
}

// pub mod xlib {
//     use log::{error, warn};
//     use std::{
//         borrow::Borrow,
//         convert::{TryFrom, TryInto},
//         ffi::CString,
//         rc::Rc,
//     };
//     use thiserror::Error;

//     use x11::xlib::{self, Atom, Window, XEvent, XInternAtom};

//     use super::{window_event::WindowEvent, WindowServerBackend};

//     #[derive(Clone)]
//     pub struct Display(Rc<*mut x11::xlib::Display>);

//     #[derive(Debug, Error)]
//     pub enum XlibError {
//         #[error("BadAccess")]
//         BadAccess,
//         #[error("BadAlloc")]
//         BadAlloc,
//         #[error("BadAtom")]
//         BadAtom,
//         #[error("BadColor")]
//         BadColor,
//         #[error("BadCursor")]
//         BadCursor,
//         #[error("BadDrawable")]
//         BadDrawable,
//         #[error("BadFont")]
//         BadFont,
//         #[error("BadGC")]
//         BadGC,
//         #[error("BadIDChoice")]
//         BadIDChoice,
//         #[error("BadImplementation")]
//         BadImplementation,
//         #[error("BadLength")]
//         BadLength,
//         #[error("BadMatch")]
//         BadMatch,
//         #[error("BadName")]
//         BadName,
//         #[error("BadPixmap")]
//         BadPixmap,
//         #[error("BadRequest")]
//         BadRequest,
//         #[error("BadValue")]
//         BadValue,
//         #[error("BadWindow")]
//         BadWindow,
//         #[error("Invalid XError: {0}")]
//         InvalidError(u8),
//     }

//     impl From<u8> for XlibError {
//         fn from(value: u8) -> Self {
//             match value {
//                 xlib::BadAccess => XlibError::BadAccess,
//                 xlib::BadAlloc => XlibError::BadAlloc,
//                 xlib::BadAtom => XlibError::BadAtom,
//                 xlib::BadColor => XlibError::BadColor,
//                 xlib::BadCursor => XlibError::BadCursor,
//                 xlib::BadDrawable => XlibError::BadDrawable,
//                 xlib::BadFont => XlibError::BadFont,
//                 xlib::BadGC => XlibError::BadGC,
//                 xlib::BadIDChoice => XlibError::BadIDChoice,
//                 xlib::BadImplementation => XlibError::BadImplementation,
//                 xlib::BadLength => XlibError::BadLength,
//                 xlib::BadMatch => XlibError::BadMatch,
//                 xlib::BadName => XlibError::BadName,
//                 xlib::BadPixmap => XlibError::BadPixmap,
//                 xlib::BadRequest => XlibError::BadRequest,
//                 xlib::BadValue => XlibError::BadValue,
//                 xlib::BadWindow => XlibError::BadWindow,
//                 any => XlibError::InvalidError(any),
//             }
//         }
//     }

//     // impl Into<i32> for XlibError {
//     //     fn into(self) -> i32 {
//     //         match self {
//     //             XlibError::BadAccess => xlib::BadAccess.into(),
//     //             XlibError::BadAlloc => xlib::BadAlloc.into(),
//     //             XlibError::BadAtom => xlib::BadAtom.into(),
//     //             XlibError::BadColor => xlib::BadColor.into(),
//     //             XlibError::BadCursor => xlib::BadCursor.into(),
//     //             XlibError::BadDrawable => xlib::BadDrawable.into(),
//     //             XlibError::BadFont => xlib::BadFont.into(),
//     //             XlibError::BadGC => xlib::BadGC.into(),
//     //             XlibError::BadIDChoice => xlib::BadIDChoice.into(),
//     //             XlibError::BadImplementation => xlib::BadImplementation.into(),
//     //             XlibError::BadLength => xlib::BadLength.into(),
//     //             XlibError::BadMatch => xlib::BadMatch.into(),
//     //             XlibError::BadName => xlib::BadName.into(),
//     //             XlibError::BadPixmap => xlib::BadPixmap.into(),
//     //             XlibError::BadRequest => xlib::BadRequest.into(),
//     //             XlibError::BadValue => xlib::BadValue.into(),
//     //             XlibError::BadWindow => xlib::BadWindow.into(),
//     //             XlibError::InvalidError(err) => err,
//     //         }
//     //     }
//     // }

//     impl Display {
//         pub fn new(display: *mut x11::xlib::Display) -> Self {
//             Self {
//                 0: Rc::new(display),
//             }
//         }

//         pub fn get(&self) -> *mut x11::xlib::Display {
//             *self.0
//         }
//     }

//     pub struct XLib {
//         display: Display,
//         root: Window,
//         screen: i32,
//         atoms: XLibAtoms,
//         keybinds: Vec<()>,
//     }

//     impl XLib {
//         fn dpy(&self) -> *mut xlib::Display {
//             self.display.get()
//         }

//         fn next_xevent(&self) -> XEvent {
//             unsafe {
//                 let mut event = std::mem::MaybeUninit::<xlib::XEvent>::zeroed()
//                     .assume_init();
//                 xlib::XNextEvent(self.dpy(), &mut event);

//                 event
//             }
//         }
//     }

//     impl TryFrom<XEvent> for WindowEvent {
//         type Error = crate::error::Error;

//         fn try_from(event: XEvent) -> Result<Self, Self::Error> {
//             match event.get_type() {
//                 xlib::MapRequest => Ok(Self::MapRequestEvent {
//                     window: event.map_request.window,
//                     event: todo!(),
//                 }),
//                 _ => Err(Self::Error::UnknownEvent),
//             }
//         }
//     }

//     impl WindowServerBackend for XLib {
//         fn next_event(&self) -> super::window_event::WindowEvent {
//             self.next_xevent().try_into().unwrap()
//         }

//         fn add_keybind(
//             &mut self,
//             keybind: super::window_event::KeyBind,
//             window: Option<u64>,
//         ) {
//             todo!()
//         }

//         fn remove_keybind(
//             &mut self,
//             keybind: super::window_event::KeyBind,
//             window: Option<u64>,
//         ) {
//             todo!()
//         }

//         fn add_mousebind(
//             &mut self,
//             keybind: super::window_event::KeyBind,
//             window: Option<u64>,
//         ) {
//             todo!()
//         }

//         fn remove_mousebind(
//             &mut self,
//             keybind: super::window_event::KeyBind,
//             window: Option<u64>,
//         ) {
//             todo!()
//         }

//         fn focus_window(&self, window: u64) {
//             todo!()
//         }

//         fn unfocus_window(&self, window: u64) {
//             todo!()
//         }

//         fn move_window(&self, window: u64, pos: i32) {
//             todo!()
//         }

//         fn resize_window(&self, window: u64, pos: i32) {
//             todo!()
//         }

//         fn hide_window(&self, window: u64) {
//             todo!()
//         }

//         fn screen_size(&self) -> (i32, i32) {
//             todo!()
//         }

//         fn kill_window(&self, window: u64) {
//             todo!()
//         }
//     }

//     struct XLibAtoms {
//         protocols: Atom,
//         delete_window: Atom,
//         active_window: Atom,
//         take_focus: Atom,
//     }

//     impl XLibAtoms {
//         fn init(display: Display) -> Self {
//             unsafe {
//                 Self {
//                     protocols: {
//                         let name = CString::new("WM_PROTOCOLS").unwrap();
//                         XInternAtom(display.get(), name.as_c_str().as_ptr(), 0)
//                     },
//                     delete_window: {
//                         let name = CString::new("WM_DELETE_WINDOW").unwrap();
//                         XInternAtom(display.get(), name.as_c_str().as_ptr(), 0)
//                     },
//                     active_window: {
//                         let name = CString::new("WM_ACTIVE_WINDOW").unwrap();
//                         XInternAtom(display.get(), name.as_c_str().as_ptr(), 0)
//                     },
//                     take_focus: {
//                         let name = CString::new("WM_TAKE_FOCUS").unwrap();
//                         XInternAtom(display.get(), name.as_c_str().as_ptr(), 0)
//                     },
//                 }
//             }
//         }
//     }

//     #[allow(dead_code)]
//     unsafe extern "C" fn xlib_error_handler(
//         _dpy: *mut x11::xlib::Display,
//         ee: *mut x11::xlib::XErrorEvent,
//     ) -> std::os::raw::c_int {
//         let err_event = ee.as_ref().unwrap();
//         let err = XlibError::from(err_event.error_code);

//         match err {
//             err @ XlibError::BadAccess
//             | err @ XlibError::BadMatch
//             | err @ XlibError::BadWindow
//             | err @ XlibError::BadDrawable => {
//                 warn!("{:?}", err);
//                 0
//             }
//             _ => {
//                 error!(
//                     "wm: fatal error:\nrequest_code: {}\nerror_code: {}",
//                     err_event.request_code, err_event.error_code
//                 );
//                 std::process::exit(1)
//             }
//         }
//     }
// }
