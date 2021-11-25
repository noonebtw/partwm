#![allow(unused_variables, dead_code)]
use log::{error, warn};
use std::{
    convert::{TryFrom, TryInto},
    ffi::CString,
    rc::Rc,
};

use thiserror::Error;

use x11::xlib::{
    self, Atom, KeyPress, KeyRelease, Window, XEvent, XInternAtom, XKeyEvent,
};

use crate::backends::window_event::ModifierKey;

use self::keysym::{virtual_keycode_to_keysym, xev_to_mouse_button, XKeySym};

use super::{
    keycodes::VirtualKeyCode,
    window_event::{
        ButtonEvent, ConfigureEvent, DestroyEvent, EnterEvent, KeyState,
        MapEvent, ModifierState, UnmapEvent, WindowEvent,
    },
    WindowServerBackend,
};

pub mod keysym;

pub type XLibWindowEvent = WindowEvent<xlib::Window>;

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
    modifier_state: ModifierState,
    root: Window,
    screen: i32,
    atoms: XLibAtoms,
    keybinds: Vec<()>,
}

impl Drop for XLib {
    fn drop(&mut self) {
        self.close_dpy();
    }
}

impl XLib {
    fn dpy(&self) -> *mut xlib::Display {
        self.display.get()
    }

    fn close_dpy(&self) {
        unsafe {
            xlib::XCloseDisplay(self.dpy());
        }
    }

    fn next_xevent(&mut self) -> XEvent {
        let event = unsafe {
            let mut event = std::mem::MaybeUninit::<xlib::XEvent>::zeroed();
            xlib::XNextEvent(self.dpy(), event.as_mut_ptr());

            event.assume_init()
        };

        match event.get_type() {
            xlib::KeyPress | xlib::KeyRelease => {
                self.update_modifier_state(AsRef::<xlib::XKeyEvent>::as_ref(
                    &event,
                ));
            }
            _ => {}
        }

        event
    }

    fn check_for_protocol(
        &self,
        window: xlib::Window,
        proto: xlib::Atom,
    ) -> bool {
        let mut protos: *mut xlib::Atom = std::ptr::null_mut();
        let mut num_protos: i32 = 0;

        unsafe {
            if xlib::XGetWMProtocols(
                self.dpy(),
                window,
                &mut protos,
                &mut num_protos,
            ) != 0
            {
                for i in 0..num_protos {
                    if *protos.offset(i as isize) == proto {
                        return true;
                    }
                }
            }
        }

        return false;
    }

    fn send_protocol(&self, window: xlib::Window, proto: Atom) -> bool {
        if self.check_for_protocol(window, proto) {
            let mut data = xlib::ClientMessageData::default();
            data.set_long(0, proto as i64);

            let mut event = XEvent {
                client_message: xlib::XClientMessageEvent {
                    type_: xlib::ClientMessage,
                    serial: 0,
                    display: self.dpy(),
                    send_event: 0,
                    window,
                    format: 32,
                    message_type: self.atoms.wm_protocols,
                    data,
                },
            };

            unsafe {
                xlib::XSendEvent(
                    self.dpy(),
                    window,
                    0,
                    xlib::NoEventMask,
                    &mut event,
                );
            }

            true
        } else {
            false
        }
    }

    #[allow(non_upper_case_globals)]
    fn update_modifier_state(&mut self, keyevent: &XKeyEvent) {
        //keyevent.keycode
        let keysym = self.keyev_to_keysym(keyevent);

        use x11::keysym::*;

        let modifier = match keysym.get() {
            XK_Shift_L | XK_Shift_R => Some(ModifierKey::Shift),
            XK_Control_L | XK_Control_R => Some(ModifierKey::Control),
            XK_Alt_L | XK_Alt_R => Some(ModifierKey::Alt),
            XK_ISO_Level3_Shift => Some(ModifierKey::AltGr),
            XK_Caps_Lock => Some(ModifierKey::ShiftLock),
            XK_Num_Lock => Some(ModifierKey::NumLock),
            XK_Win_L | XK_Win_R => Some(ModifierKey::Super),
            XK_Super_L | XK_Super_R => Some(ModifierKey::Super),
            _ => None,
        };

        if let Some(modifier) = modifier {
            match keyevent.type_ {
                KeyPress => self.modifier_state.set_mod(modifier),
                KeyRelease => self.modifier_state.unset_mod(modifier),
                _ => unreachable!("keyyevent != (KeyPress | KeyRelease)"),
            }
        }
    }

    fn vk_to_keycode(&self, vk: VirtualKeyCode) -> i32 {
        unsafe {
            xlib::XKeysymToKeycode(
                self.dpy(),
                virtual_keycode_to_keysym(vk).unwrap() as u64,
            ) as i32
        }
    }

    fn keyev_to_keysym(&self, ev: &XKeyEvent) -> XKeySym {
        let keysym =
            unsafe { xlib::XLookupKeysym(ev as *const _ as *mut _, 0) };

        XKeySym::new(keysym as u32)
    }
}

impl TryFrom<XEvent> for XLibWindowEvent {
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
                    position: (ev.x, ev.y).into(),
                    size: (ev.width, ev.height).into(),
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
            // both ButtonPress and ButtonRelease use the XButtonEvent structure, aliased as either
            // XButtonReleasedEvent or XButtonPressedEvent
            xlib::ButtonPress | xlib::ButtonRelease => {
                let ev = unsafe { &event.button };
                let keycode = xev_to_mouse_button(ev).unwrap();
                let state = if ev.state as i32 == xlib::ButtonPress {
                    KeyState::Pressed
                } else {
                    KeyState::Released
                };

                let modifierstate = ModifierState::empty();

                Ok(Self::ButtonEvent(ButtonEvent::new(
                    ev.subwindow,
                    state,
                    keycode,
                    (ev.x, ev.y).into(),
                    modifierstate,
                )))
            }
            _ => Err(Self::Error::UnknownEvent),
        }
    }
}

impl WindowServerBackend for XLib {
    type Window = xlib::Window;

    fn next_event(&mut self) -> super::window_event::WindowEvent<Self::Window> {
        std::iter::from_fn(|| {
            TryInto::<XLibWindowEvent>::try_into(self.next_xevent()).ok()
        })
        .next()
        .unwrap()
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

    fn move_window(&self, window: Self::Window, new_pos: (i32, i32)) {
        todo!()
    }

    fn resize_window(&self, window: Self::Window, new_pos: (i32, i32)) {
        todo!()
    }

    fn hide_window(&self, window: Self::Window) {
        todo!()
    }

    fn screen_size(&self) -> (i32, i32) {
        unsafe {
            let mut wa =
                std::mem::MaybeUninit::<xlib::XWindowAttributes>::zeroed();

            xlib::XGetWindowAttributes(self.dpy(), self.root, wa.as_mut_ptr());

            let wa = wa.assume_init();

            (wa.width, wa.height)
        }
    }

    fn kill_window(&self, window: Self::Window) {
        if !self.send_protocol(window, self.atoms.wm_delete_window) {
            unsafe {
                xlib::XKillClient(self.dpy(), window);
            }
        }
        todo!()
    }

    fn raise_window(&self, window: Self::Window) {
        todo!()
    }

    fn get_parent_window(&self, window: Self::Window) -> Option<Self::Window> {
        todo!()
    }

    fn get_window_size(&self, window: Self::Window) -> Option<(i32, i32)> {
        todo!()
    }

    fn new() -> Self {
        todo!()
    }
}

struct XLibAtoms {
    wm_protocols: Atom,
    wm_delete_window: Atom,
    wm_active_window: Atom,
    wm_take_focus: Atom,
    net_supported: Atom,
    net_active_window: Atom,
    net_client_list: Atom,
    net_wm_name: Atom,
    net_wm_state: Atom,
    net_wm_state_fullscreen: Atom,
    net_wm_window_type: Atom,
    net_wm_window_type_dialog: Atom,
}

impl XLibAtoms {
    fn init(display: Display) -> Self {
        Self {
            wm_protocols: Self::get_atom(&display, "WM_PROTOCOLS").unwrap(),
            wm_delete_window: Self::get_atom(&display, "WM_DELETE_WINDOW")
                .unwrap(),
            wm_active_window: Self::get_atom(&display, "WM_ACTIVE_WINDOW")
                .unwrap(),
            wm_take_focus: Self::get_atom(&display, "WM_TAKE_FOCUS").unwrap(),
            net_supported: Self::get_atom(&display, "_NET_SUPPORTED").unwrap(),
            net_active_window: Self::get_atom(&display, "_NET_ACTIVE_WINDOW")
                .unwrap(),
            net_client_list: Self::get_atom(&display, "_NET_CLIENT_LIST")
                .unwrap(),
            net_wm_name: Self::get_atom(&display, "_NET_WM_NAME").unwrap(),
            net_wm_state: Self::get_atom(&display, "_NET_WM_STATE").unwrap(),
            net_wm_state_fullscreen: Self::get_atom(
                &display,
                "_NET_WM_STATE_FULLSCREEN",
            )
            .unwrap(),
            net_wm_window_type: Self::get_atom(&display, "_NET_WM_WINDOW_TYPE")
                .unwrap(),
            net_wm_window_type_dialog: Self::get_atom(
                &display,
                "_NET_WM_WINDOW_TYPE_DIALOG",
            )
            .unwrap(),
        }
    }

    fn get_atom(display: &Display, atom: &str) -> Option<Atom> {
        let name = CString::new(atom).ok()?;
        match unsafe { XInternAtom(display.get(), name.as_c_str().as_ptr(), 0) }
        {
            0 => None,
            atom => Some(atom),
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
