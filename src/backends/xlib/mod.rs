use log::{debug, error, warn};
use num_traits::Zero;
use std::{ffi::CString, mem::MaybeUninit, ptr::NonNull, rc::Rc};
use std::{ffi::CString, rc::Rc};
>>>>>>> variant B
use std::{ffi::CString, mem::MaybeUninit, ptr::NonNull, rc::Rc};
======= end

use thiserror::Error;

use x11::xlib::{self, Atom, Success, Window, XEvent, XInternAtom, XKeyEvent};

use crate::backends::{
    keycodes::KeyOrButton, xlib::keysym::mouse_button_to_xbutton,
};

use self::keysym::{
    keysym_to_virtual_keycode, virtual_keycode_to_keysym, xev_to_mouse_button,
    XKeySym,
};

use super::{
    keycodes::VirtualKeyCode,
    window_event::{
        ButtonEvent, ConfigureEvent, DestroyEvent, EnterEvent, FullscreenEvent,
        FullscreenState, KeyEvent, KeyOrMouseBind, KeyState, MapEvent,
        ModifierState, MotionEvent, UnmapEvent, WindowEvent,
    },
    WindowServerBackend,
};
use crate::util::{Point, Size};

pub mod color;
pub mod keysym;

pub type XLibWindowEvent = WindowEvent<xlib::Window>;

#[derive(Clone)]
pub struct Display(Rc<NonNull<x11::xlib::Display>>);

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

pub mod ewmh {
    use std::ffi::CString;

    use strum::{EnumCount, EnumIter};
    use x11::xlib::Atom;

    use super::Display;

    #[derive(Debug, PartialEq, Eq, EnumIter, EnumCount, Clone, Copy)]
    pub enum EWMHAtoms {
        NetSupported,
        NetClientList,
        NetNumberOfDesktops,
        NetDesktopGeometry,
        NetDesktopViewport,
        NetCurrentDesktop,
        NetDesktopNames,
        NetActiveWindow,
        NetWorkarea,
        NetSupportingWmCheck,
        NetVirtualRoots,
        NetDesktopLayout,
        NetShowingDesktop,
        NetCloseWindow,
        NetMoveresizeWindow,
        NetWmMoveresize,
        NetRestackWindow,
        NetRequestFrameExtents,
        NetWmName,
        NetWmVisibleName,
        NetWmIconName,
        NetWmVisibleIconName,
        NetWmDesktop,
        NetWmWindowType,
        NetWmState,
        NetWmAllowedActions,
        NetWmStrut,
        NetWmStrutPartial,
        NetWmIconGeometry,
        NetWmIcon,
        NetWmPid,
        NetWmHandledIcons,
        NetWmUserTime,
        NetFrameExtents,
        NetWmPing,
        NetWmSyncRequest,

        // idk if these are atoms?
        NetWmWindowTypeDesktop,
        NetWmWindowTypeDock,
        NetWmWindowTypeToolbar,
        NetWmWindowTypeMenu,
        NetWmWindowTypeUtility,
        NetWmWindowTypeSplash,
        NetWmWindowTypeDialog,
        NetWmWindowTypeNormal,
        NetWmStateModal,
        NetWmStateSticky,
        NetWmStateMaximizedVert,
        NetWmStateMaximizedHorz,
        NetWmStateShaded,
        NetWmStateSkipTaskbar,
        NetWmStateSkipPager,
        NetWmStateHidden,
        NetWmStateFullscreen,
        NetWmStateAbove,
        NetWmStateBelow,
        NetWmStateDemandsAttention,
        NetWmActionMove,
        NetWmActionResize,
        NetWmActionMinimize,
        NetWmActionShade,
        NetWmActionStick,
        NetWmActionMaximizeHorz,
        NetWmActionMaximizeVert,
        NetWmActionFullscreen,
        NetWmActionChangeDesktop,
        NetWmActionClose,
    }

    impl EWMHAtoms {
        pub fn try_get_atoms(display: Display) -> Option<Vec<Atom>> {
            use strum::IntoEnumIterator;
            Self::iter()
                .map(|atom| atom.try_into_x_atom(&display))
                .collect::<Option<Vec<_>>>()
        }

        fn try_into_x_atom(self, display: &Display) -> Option<Atom> {
            let name = CString::new::<&str>(self.into()).ok()?;
            match unsafe {
                x11::xlib::XInternAtom(
                    display.get(),
                    name.as_c_str().as_ptr(),
                    0,
                )
            } {
                0 => None,
                atom => Some(atom),
            }
        }
    }

    impl From<EWMHAtoms> for u8 {
        fn from(atom: EWMHAtoms) -> Self {
            atom as u8
        }
    }

    impl From<EWMHAtoms> for &str {
        fn from(atom: EWMHAtoms) -> Self {
            match atom {
                EWMHAtoms::NetSupported => "_NET_SUPPORTED",
                EWMHAtoms::NetClientList => "_NET_CLIENT_LIST",
                EWMHAtoms::NetNumberOfDesktops => "_NET_NUMBER_OF_DESKTOPS",
                EWMHAtoms::NetDesktopGeometry => "_NET_DESKTOP_GEOMETRY",
                EWMHAtoms::NetDesktopViewport => "_NET_DESKTOP_VIEWPORT",
                EWMHAtoms::NetCurrentDesktop => "_NET_CURRENT_DESKTOP",
                EWMHAtoms::NetDesktopNames => "_NET_DESKTOP_NAMES",
                EWMHAtoms::NetActiveWindow => "_NET_ACTIVE_WINDOW",
                EWMHAtoms::NetWorkarea => "_NET_WORKAREA",
                EWMHAtoms::NetSupportingWmCheck => "_NET_SUPPORTING_WM_CHECK",
                EWMHAtoms::NetVirtualRoots => "_NET_VIRTUAL_ROOTS",
                EWMHAtoms::NetDesktopLayout => "_NET_DESKTOP_LAYOUT",
                EWMHAtoms::NetShowingDesktop => "_NET_SHOWING_DESKTOP",
                EWMHAtoms::NetCloseWindow => "_NET_CLOSE_WINDOW",
                EWMHAtoms::NetMoveresizeWindow => "_NET_MOVERESIZE_WINDOW",
                EWMHAtoms::NetWmMoveresize => "_NET_WM_MOVERESIZE",
                EWMHAtoms::NetRestackWindow => "_NET_RESTACK_WINDOW",
                EWMHAtoms::NetRequestFrameExtents => {
                    "_NET_REQUEST_FRAME_EXTENTS"
                }
                EWMHAtoms::NetWmName => "_NET_WM_NAME",
                EWMHAtoms::NetWmVisibleName => "_NET_WM_VISIBLE_NAME",
                EWMHAtoms::NetWmIconName => "_NET_WM_ICON_NAME",
                EWMHAtoms::NetWmVisibleIconName => "_NET_WM_VISIBLE_ICON_NAME",
                EWMHAtoms::NetWmDesktop => "_NET_WM_DESKTOP",
                EWMHAtoms::NetWmWindowType => "_NET_WM_WINDOW_TYPE",
                EWMHAtoms::NetWmState => "_NET_WM_STATE",
                EWMHAtoms::NetWmAllowedActions => "_NET_WM_ALLOWED_ACTIONS",
                EWMHAtoms::NetWmStrut => "_NET_WM_STRUT",
                EWMHAtoms::NetWmStrutPartial => "_NET_WM_STRUT_PARTIAL",
                EWMHAtoms::NetWmIconGeometry => "_NET_WM_ICON_GEOMETRY",
                EWMHAtoms::NetWmIcon => "_NET_WM_ICON",
                EWMHAtoms::NetWmPid => "_NET_WM_PID",
                EWMHAtoms::NetWmHandledIcons => "_NET_WM_HANDLED_ICONS",
                EWMHAtoms::NetWmUserTime => "_NET_WM_USER_TIME",
                EWMHAtoms::NetFrameExtents => "_NET_FRAME_EXTENTS",
                EWMHAtoms::NetWmPing => "_NET_WM_PING",
                EWMHAtoms::NetWmSyncRequest => "_NET_WM_SYNC_REQUEST",
                EWMHAtoms::NetWmWindowTypeDesktop => {
                    "_NET_WM_WINDOW_TYPE_DESKTOP"
                }
                EWMHAtoms::NetWmWindowTypeDock => "_NET_WM_WINDOW_TYPE_DOCK",
                EWMHAtoms::NetWmWindowTypeToolbar => {
                    "_NET_WM_WINDOW_TYPE_TOOLBAR"
                }
                EWMHAtoms::NetWmWindowTypeMenu => "_NET_WM_WINDOW_TYPE_MENU",
                EWMHAtoms::NetWmWindowTypeUtility => {
                    "_NET_WM_WINDOW_TYPE_UTILITY"
                }
                EWMHAtoms::NetWmWindowTypeSplash => {
                    "_NET_WM_WINDOW_TYPE_SPLASH"
                }
                EWMHAtoms::NetWmWindowTypeDialog => {
                    "_NET_WM_WINDOW_TYPE_DIALOG"
                }
                EWMHAtoms::NetWmWindowTypeNormal => {
                    "_NET_WM_WINDOW_TYPE_NORMAL"
                }
                EWMHAtoms::NetWmStateModal => "_NET_WM_STATE_MODAL",
                EWMHAtoms::NetWmStateSticky => "_NET_WM_STATE_STICKY",
                EWMHAtoms::NetWmStateMaximizedVert => {
                    "_NET_WM_STATE_MAXIMIZED_VERT"
                }
                EWMHAtoms::NetWmStateMaximizedHorz => {
                    "_NET_WM_STATE_MAXIMIZED_HORZ"
                }
                EWMHAtoms::NetWmStateShaded => "_NET_WM_STATE_SHADED",
                EWMHAtoms::NetWmStateSkipTaskbar => {
                    "_NET_WM_STATE_SKIP_TASKBAR"
                }
                EWMHAtoms::NetWmStateSkipPager => "_NET_WM_STATE_SKIP_PAGER",
                EWMHAtoms::NetWmStateHidden => "_NET_WM_STATE_HIDDEN",
                EWMHAtoms::NetWmStateFullscreen => "_NET_WM_STATE_FULLSCREEN",
                EWMHAtoms::NetWmStateAbove => "_NET_WM_STATE_ABOVE",
                EWMHAtoms::NetWmStateBelow => "_NET_WM_STATE_BELOW",
                EWMHAtoms::NetWmStateDemandsAttention => {
                    "_NET_WM_STATE_DEMANDS_ATTENTION"
                }
                EWMHAtoms::NetWmActionMove => "_NET_WM_ACTION_MOVE",
                EWMHAtoms::NetWmActionResize => "_NET_WM_ACTION_RESIZE",
                EWMHAtoms::NetWmActionMinimize => "_NET_WM_ACTION_MINIMIZE",
                EWMHAtoms::NetWmActionShade => "_NET_WM_ACTION_SHADE",
                EWMHAtoms::NetWmActionStick => "_NET_WM_ACTION_STICK",
                EWMHAtoms::NetWmActionMaximizeHorz => {
                    "_NET_WM_ACTION_MAXIMIZE_HORZ"
                }
                EWMHAtoms::NetWmActionMaximizeVert => {
                    "_NET_WM_ACTION_MAXIMIZE_VERT"
                }
                EWMHAtoms::NetWmActionFullscreen => "_NET_WM_ACTION_FULLSCREEN",
                EWMHAtoms::NetWmActionChangeDesktop => {
                    "_NET_WM_ACTION_CHANGE_DESKTOP"
                }
                EWMHAtoms::NetWmActionClose => "_NET_WM_ACTION_CLOSE",
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn get_atoms() {
            let display = Display::open().unwrap();
            let atoms = EWMHAtoms::try_get_atoms(display).expect("atoms");
            println!("{:?}", atoms);
        }
    }
}

impl Display {
    pub fn new(display: *mut x11::xlib::Display) -> Option<Self> {
        NonNull::new(display).map(|ptr| Self(Rc::new(ptr)))
    }

    // TODO: error communication
    pub fn open() -> Option<Self> {
        Self::new(unsafe { xlib::XOpenDisplay(std::ptr::null()) })
    }

    /// this should definitely be unsafe lmao
    pub fn get(&self) -> *mut x11::xlib::Display {
        self.0.as_ptr()
    }
}

pub struct XLib {
    display: Display,
    root: Window,
    screen: i32,
    atoms: XLibAtoms,
    keybinds: Vec<KeyOrMouseBind>,
    active_border_color: Option<color::XftColor>,
    inactive_border_color: Option<color::XftColor>,
}

impl Drop for XLib {
    fn drop(&mut self) {
        self.close_dpy();
    }
}

impl XLib {
    fn new() -> Self {
        let (display, screen, root) = {
            let display = Display::open().expect("failed to open x display");
            let screen = unsafe { xlib::XDefaultScreen(display.get()) };
            let root = unsafe { xlib::XRootWindow(display.get(), screen) };

            (display, screen, root)
        };

        let atoms = XLibAtoms::init(display.clone());

        Self {
            display,
            screen,
            root,
            atoms,
            keybinds: Vec::new(),
            active_border_color: None,
            inactive_border_color: None,
        }
    }

    unsafe fn init_as_wm(&self) {
        let mut window_attributes =
            std::mem::MaybeUninit::<xlib::XSetWindowAttributes>::zeroed()
                .assume_init();

        window_attributes.event_mask = xlib::SubstructureRedirectMask
            | xlib::StructureNotifyMask
            | xlib::SubstructureNotifyMask
            | xlib::EnterWindowMask
            | xlib::PointerMotionMask
            | xlib::ButtonPressMask;

        xlib::XChangeWindowAttributes(
            self.dpy(),
            self.root,
            xlib::CWEventMask,
            &mut window_attributes,
        );

        xlib::XSelectInput(self.dpy(), self.root, window_attributes.event_mask);
        xlib::XSetErrorHandler(Some(xlib_error_handler));
        xlib::XSync(self.dpy(), 0);
    }

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

        // match event.get_type() {
        //     xlib::KeyPress | xlib::KeyRelease => {
        //         self.update_modifier_state(AsRef::<xlib::XKeyEvent>::as_ref(
        //             &event,
        //         ));
        //     }
        //     _ => {}
        // }

        event
    }

    fn xevent_to_window_event(&self, event: XEvent) -> Option<XLibWindowEvent> {
        match event.get_type() {
            xlib::MapRequest => {
                let ev = unsafe { &event.map_request };
                Some(XLibWindowEvent::MapRequestEvent(MapEvent {
                    window: ev.window,
                }))
            }
            xlib::UnmapNotify => {
                let ev = unsafe { &event.unmap };
                Some(XLibWindowEvent::UnmapEvent(UnmapEvent {
                    window: ev.window,
                }))
            }
            xlib::ConfigureRequest => {
                let ev = unsafe { &event.configure_request };
                Some(XLibWindowEvent::ConfigureEvent(ConfigureEvent {
                    window: ev.window,
                    position: (ev.x, ev.y).into(),
                    size: (ev.width, ev.height).into(),
                }))
            }
            xlib::EnterNotify => {
                let ev = unsafe { &event.crossing };
                Some(XLibWindowEvent::EnterEvent(EnterEvent {
                    window: ev.window,
                }))
            }
            xlib::DestroyNotify => {
                let ev = unsafe { &event.destroy_window };
                Some(XLibWindowEvent::DestroyEvent(DestroyEvent {
                    window: ev.window,
                }))
            }
            xlib::MotionNotify => {
                let ev = unsafe { &event.motion };
                Some(XLibWindowEvent::MotionEvent(MotionEvent {
                    position: (ev.x, ev.y).into(),
                    window: ev.window,
                }))
            }
            // both ButtonPress and ButtonRelease use the XButtonEvent structure, aliased as either
            // XButtonReleasedEvent or XButtonPressedEvent
            xlib::ButtonPress | xlib::ButtonRelease => {
                let ev = unsafe { &event.button };
                let keycode = xev_to_mouse_button(ev).unwrap();
                let state = if ev.type_ == xlib::ButtonPress {
                    KeyState::Pressed
                } else {
                    KeyState::Released
                };

                Some(XLibWindowEvent::ButtonEvent(ButtonEvent::new(
                    ev.subwindow,
                    state,
                    keycode,
                    (ev.x, ev.y).into(),
                    ModifierState::from_modmask(ev.state),
                )))
            }
            xlib::KeyPress | xlib::KeyRelease => {
                let ev = unsafe { &event.key };

                let keycode =
                    keysym_to_virtual_keycode(self.keyev_to_keysym(ev).get());
                let state = if ev.type_ == xlib::KeyPress {
                    KeyState::Pressed
                } else {
                    KeyState::Released
                };

                keycode.map(|keycode| {
                    XLibWindowEvent::KeyEvent(KeyEvent::new(
                        ev.subwindow,
                        state,
                        keycode,
                        ModifierState::from_modmask(ev.state),
                    ))
                })
            }
            xlib::PropertyNotify => {
                let ev = unsafe { &event.property };

                match ev.atom {
                    atom if atom == self.atoms.net_wm_window_type => {
                        if self
                            .get_atom_property(
                                ev.window,
                                self.atoms.net_wm_state,
                            )
                            .map(|atom| {
                                *atom == self.atoms.net_wm_state_fullscreen
                            })
                            .unwrap_or(false)
                        {
                            debug!("fullscreen event");
                            Some(XLibWindowEvent::FullscreenEvent(
                                FullscreenEvent::new(
                                    ev.window,
                                    FullscreenState::On,
                                ),
                            ))
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            xlib::ClientMessage => {
                let ev = unsafe { &event.client_message };

                match ev.message_type {
                    message_type if message_type == self.atoms.net_wm_state => {
                        let data = ev.data.as_longs();
                        if data[1] as u64 == self.atoms.net_wm_state_fullscreen
                            || data[2] as u64
                                == self.atoms.net_wm_state_fullscreen
                        {
                            debug!("fullscreen event");
                            Some(XLibWindowEvent::FullscreenEvent(
                                FullscreenEvent::new(
                                    ev.window,
                                    match data[0] /* as u64 */ {
                                        0 => FullscreenState::Off,
                                        1 => FullscreenState::On,
                                        2 => FullscreenState::Toggle,
                                        _ => {
                                            unreachable!()
                                        }
                                    },
                                ),
                            ))
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    #[allow(dead_code)]
    fn get_window_attributes(
        &self,
        window: xlib::Window,
    ) -> Option<xlib::XWindowAttributes> {
        let mut wa = unsafe {
            std::mem::MaybeUninit::<xlib::XWindowAttributes>::zeroed()
                .assume_init()
        };

        if unsafe {
            xlib::XGetWindowAttributes(self.dpy(), window, &mut wa) != 0
        } {
            Some(wa)
        } else {
            None
        }
    }

    fn get_atom_property(
        &self,
        window: xlib::Window,
        atom: xlib::Atom,
    ) -> Option<xpointer::XPointer<xlib::Atom>> {
        let mut di = 0;
        let mut dl0 = 0;
        let mut dl1 = 0;
        let mut da = 0;

        let (atom_out, success) =
            xpointer::XPointer::<xlib::Atom>::build_with_result(|ptr| unsafe {
                xlib::XGetWindowProperty(
                    self.dpy(),
                    window,
                    atom,
                    0,
                    std::mem::size_of::<xlib::Atom>() as i64,
                    0,
                    xlib::XA_ATOM,
                    &mut da,
                    &mut di,
                    &mut dl0,
                    &mut dl1,
                    ptr as *mut _ as *mut _,
                ) == Success.into()
            });

        debug!("get_atom_property: {} {:?}", success, atom_out);

        success.then(|| atom_out).flatten()
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

    // #[allow(non_upper_case_globals)]
    // fn update_modifier_state(&mut self, keyevent: &XKeyEvent) {
    //     //keyevent.keycode
    //     let keysym = self.keyev_to_keysym(keyevent);

    //     use x11::keysym::*;

    //     let modifier = match keysym.get() {
    //         XK_Shift_L | XK_Shift_R => Some(ModifierKey::Shift),
    //         XK_Control_L | XK_Control_R => Some(ModifierKey::Control),
    //         XK_Alt_L | XK_Alt_R => Some(ModifierKey::Alt),
    //         XK_ISO_Level3_Shift => Some(ModifierKey::AltGr),
    //         XK_Caps_Lock => Some(ModifierKey::ShiftLock),
    //         XK_Num_Lock => Some(ModifierKey::NumLock),
    //         XK_Win_L | XK_Win_R => Some(ModifierKey::Super),
    //         XK_Super_L | XK_Super_R => Some(ModifierKey::Super),
    //         _ => None,
    //     };

    //     if let Some(modifier) = modifier {
    //         match keyevent.type_ {
    //             KeyPress => self.modifier_state.insert_mod(modifier),
    //             KeyRelease => self.modifier_state.unset_mod(modifier),
    //             _ => unreachable!("keyyevent != (KeyPress | KeyRelease)"),
    //         }
    //     }
    // }

    fn get_numlock_mask(&self) -> Option<u32> {
        unsafe {
            let modmap = xlib::XGetModifierMapping(self.dpy());
            let max_keypermod = (*modmap).max_keypermod;

            for i in 0..8 {
                for j in 0..max_keypermod {
                    if *(*modmap)
                        .modifiermap
                        .offset((i * max_keypermod + j) as isize)
                        == xlib::XKeysymToKeycode(
                            self.dpy(),
                            x11::keysym::XK_Num_Lock as u64,
                        )
                    {
                        return Some(1 << i);
                    }
                }
            }
        }

        None
    }

    fn grab_key_or_button(
        &self,
        binding: &KeyOrMouseBind,
        window: xlib::Window,
    ) {
        let modmask = binding.modifiers.as_modmask(self);

        let numlock_mask = self
            .get_numlock_mask()
            .expect("failed to query numlock mask.");

        let modifiers = vec![
            0,
            xlib::LockMask,
            numlock_mask,
            xlib::LockMask | numlock_mask,
        ];

        let keycode = match binding.key {
            KeyOrButton::Key(key) => self.vk_to_keycode(key),
            KeyOrButton::Button(button) => mouse_button_to_xbutton(button),
        };

        for modifier in modifiers.iter() {
            match binding.key {
                KeyOrButton::Key(_) => unsafe {
                    xlib::XGrabKey(
                        self.dpy(),
                        keycode,
                        modmask | modifier,
                        window,
                        1,
                        xlib::GrabModeAsync,
                        xlib::GrabModeAsync,
                    );
                },
                KeyOrButton::Button(_) => unsafe {
                    xlib::XGrabButton(
                        self.dpy(),
                        keycode as u32,
                        modmask | modifier,
                        window,
                        1,
                        (xlib::ButtonPressMask
                            | xlib::ButtonReleaseMask
                            | xlib::PointerMotionMask)
                            as u32,
                        xlib::GrabModeAsync,
                        xlib::GrabModeAsync,
                        0,
                        0,
                    );
                },
            }
        }
    }

    #[allow(dead_code)]
    fn ungrab_key_or_button(
        &self,
        binding: &KeyOrMouseBind,
        window: xlib::Window,
    ) {
        let modmask = binding.modifiers.as_modmask(self);

        let numlock_mask = self
            .get_numlock_mask()
            .expect("failed to query numlock mask.");

        let modifiers = vec![
            0,
            xlib::LockMask,
            numlock_mask,
            xlib::LockMask | numlock_mask,
        ];

        let keycode = match binding.key {
            KeyOrButton::Key(key) => self.vk_to_keycode(key),
            KeyOrButton::Button(button) => mouse_button_to_xbutton(button),
        };

        for modifier in modifiers.iter() {
            match binding.key {
                KeyOrButton::Key(_) => unsafe {
                    xlib::XUngrabKey(
                        self.dpy(),
                        keycode,
                        modmask | modifier,
                        window,
                    );
                },
                KeyOrButton::Button(_) => unsafe {
                    xlib::XUngrabButton(
                        self.dpy(),
                        keycode as u32,
                        modmask | modifier,
                        window,
                    );
                },
            }
        }
    }

    fn grab_global_keybinds(&self, window: xlib::Window) {
        for binding in self.keybinds.iter() {
            self.grab_key_or_button(binding, window);
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

trait ModifierStateExt {
    fn as_modmask(&self, xlib: &XLib) -> u32;
    fn from_modmask(modmask: u32) -> Self;
}

impl ModifierStateExt for ModifierState {
    fn as_modmask(&self, xlib: &XLib) -> u32 {
        let mut mask = 0;
        let _numlock_mask = xlib
            .get_numlock_mask()
            .expect("failed to query numlock mask");

        mask |= xlib::ShiftMask * u32::from(self.contains(Self::SHIFT));
        //mask |= xlib::LockMask * u32::from(self.contains(Self::SHIFT_LOCK));
        mask |= xlib::ControlMask * u32::from(self.contains(Self::CONTROL));
        mask |= xlib::Mod1Mask * u32::from(self.contains(Self::ALT));
        //mask |= xlib::Mod2Mask * u32::from(self.contains(Self::NUM_LOCK));
        //mask |= xlib::Mod3Mask * u32::from(self.contains(Self::ALT_GR));
        mask |= xlib::Mod4Mask * u32::from(self.contains(Self::SUPER));
        //mask |= numlock_mask * u32::from(self.contains(Self::NUM_LOCK));

        mask
    }

    fn from_modmask(modmask: u32) -> Self {
        let mut state = Self::empty();
        state.set(Self::SHIFT, (modmask & xlib::ShiftMask) != 0);
        //state.set(Self::SHIFT_LOCK, (modmask & xlib::LockMask) != 0);
        state.set(Self::CONTROL, (modmask & xlib::ControlMask) != 0);
        state.set(Self::ALT, (modmask & xlib::Mod1Mask) != 0);
        //state.set(Self::NUM_LOCK, (modmask & xlib::Mod2Mask) != 0);
        state.set(Self::ALT_GR, (modmask & xlib::Mod3Mask) != 0);
        state.set(Self::SUPER, (modmask & xlib::Mod4Mask) != 0);

        state
    }
}

impl WindowServerBackend for XLib {
    type Window = xlib::Window;

    fn build() -> Self {
        let xlib = Self::new();
        unsafe { xlib.init_as_wm() };
        xlib
    }

    fn next_event(&mut self) -> super::window_event::WindowEvent<Self::Window> {
        loop {
            let ev = self.next_xevent();
            let ev = self.xevent_to_window_event(ev);

            if let Some(ev) = ev {
                return ev;
            }
        }
    }

    fn handle_event(
        &mut self,
        event: super::window_event::WindowEvent<Self::Window>,
    ) {
        match event {
            WindowEvent::MapRequestEvent(event) => {
                unsafe {
                    xlib::XMapWindow(self.dpy(), event.window);

                    xlib::XSelectInput(
                        self.dpy(),
                        event.window,
                        xlib::EnterWindowMask
                            | xlib::FocusChangeMask
                            | xlib::PropertyChangeMask
                            | xlib::StructureNotifyMask,
                    );
                }

                self.grab_global_keybinds(event.window);
            }
            WindowEvent::ConfigureEvent(event) => {
                self.configure_window(
                    event.window,
                    Some(event.size),
                    Some(event.position),
                    None,
                );
            }
            _ => {}
        }
    }

    fn add_keybind(&mut self, keybind: super::window_event::KeyOrMouseBind) {
        self.grab_key_or_button(&keybind, self.root);
        self.keybinds.push(keybind);
    }

    fn remove_keybind(
        &mut self,
        keybind: &super::window_event::KeyOrMouseBind,
    ) {
        self.keybinds.retain(|kb| kb != keybind);
    }

    fn focus_window(&self, window: Self::Window) {
        unsafe {
            xlib::XSetInputFocus(
                self.dpy(),
                window,
                xlib::RevertToPointerRoot,
                xlib::CurrentTime,
            );

            let border_color = self
                .active_border_color
                .as_ref()
                .map(|color| color.pixel())
                .unwrap_or_else(|| {
                    xlib::XDefaultScreenOfDisplay(self.dpy())
                        .as_ref()
                        .unwrap()
                        .white_pixel
                });

            xlib::XSetWindowBorder(self.dpy(), window, border_color);

            xlib::XChangeProperty(
                self.dpy(),
                self.root,
                self.atoms.wm_active_window,
                xlib::XA_WINDOW,
                32,
                xlib::PropModeReplace,
                &window as *const u64 as *const _,
                1,
            );
        }

        self.send_protocol(window, self.atoms.wm_take_focus);
    }

    fn unfocus_window(&self, window: Self::Window) {
        unsafe {
            xlib::XSetInputFocus(
                self.dpy(),
                self.root,
                xlib::RevertToPointerRoot,
                xlib::CurrentTime,
            );

            // TODO: make painting the window border a seperate function, and configurable

            let border_color = self
                .inactive_border_color
                .as_ref()
                .map(|color| color.pixel())
                .unwrap_or_else(|| {
                    xlib::XDefaultScreenOfDisplay(self.dpy())
                        .as_ref()
                        .unwrap()
                        .black_pixel
                });

            xlib::XSetWindowBorder(self.dpy(), window, border_color);

            xlib::XDeleteProperty(
                self.dpy(),
                self.root,
                self.atoms.wm_active_window,
            );
        }
    }

    fn raise_window(&self, window: Self::Window) {
        unsafe {
            xlib::XRaiseWindow(self.dpy(), window);
        }
    }

    fn hide_window(&self, window: Self::Window) {
        let screen_size = self.screen_size() + Size::new(100, 100);
        self.move_window(window, screen_size.into());
    }

    fn kill_window(&self, window: Self::Window) {
        if !self.send_protocol(window, self.atoms.wm_delete_window) {
            unsafe {
                xlib::XKillClient(self.dpy(), window);
            }
        }
    }

    fn get_parent_window(&self, window: Self::Window) -> Option<Self::Window> {
        let mut parent_window: Self::Window = 0;
        if unsafe {
            xlib::XGetTransientForHint(self.dpy(), window, &mut parent_window)
                != 0
        } {
            Some(parent_window)
        } else {
            None
        }
    }

    fn configure_window(
        &self,
        window: Self::Window,
        new_size: Option<crate::util::Size<i32>>,
        new_pos: Option<crate::util::Point<i32>>,
        new_border: Option<i32>,
    ) {
        let position = new_pos.unwrap_or(Point::zero());
        let size = new_size.unwrap_or(Size::zero());
        let mut wc = xlib::XWindowChanges {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
            border_width: new_border.unwrap_or(0),
            sibling: 0,
            stack_mode: 0,
        };

        let mask = {
            let mut mask = 0;
            if new_pos.is_some() {
                mask |= xlib::CWX | xlib::CWY;
            }
            if new_size.is_some() && wc.width > 1 && wc.height > 1 {
                mask |= xlib::CWWidth | xlib::CWHeight;
            }
            if new_border.is_some() {
                mask |= xlib::CWBorderWidth;
            }

            u32::from(mask)
        };

        unsafe {
            xlib::XConfigureWindow(self.dpy(), window, mask, &mut wc);
        }
    }

    fn screen_size(&self) -> Size<i32> {
        unsafe {
            let mut wa =
                std::mem::MaybeUninit::<xlib::XWindowAttributes>::zeroed();

            xlib::XGetWindowAttributes(self.dpy(), self.root, wa.as_mut_ptr());

            let wa = wa.assume_init();

            (wa.width, wa.height).into()
        }
    }

    fn get_window_size(&self, window: Self::Window) -> Option<Size<i32>> {
        self.get_window_attributes(window)
            .map(|wa| (wa.width, wa.height).into())
    }

    fn grab_cursor(&self) {
        unsafe {
            xlib::XGrabPointer(
                self.dpy(),
                self.root,
                0,
                (xlib::ButtonPressMask
                    | xlib::ButtonReleaseMask
                    | xlib::PointerMotionMask) as u32,
                xlib::GrabModeAsync,
                xlib::GrabModeAsync,
                0,
                0,
                xlib::CurrentTime,
            );
        }
    }

    fn ungrab_cursor(&self) {
        unsafe {
            xlib::XUngrabPointer(self.dpy(), xlib::CurrentTime);
        }
    }

    fn move_cursor(&self, window: Option<Self::Window>, position: Point<i32>) {
        unsafe {
            xlib::XWarpPointer(
                self.dpy(),
                0,
                window.unwrap_or(self.root),
                0,
                0,
                0,
                0,
                position.x,
                position.y,
            );
        }
    }

    fn all_windows(&self) -> Option<Vec<Self::Window>> {
        let mut parent = 0;
        let mut root = 0;
        let mut children = std::ptr::null_mut();
        let mut num_children = 0;

        unsafe {
            xlib::XQueryTree(
                self.dpy(),
                self.root,
                &mut root,
                &mut parent,
                &mut children,
                &mut num_children,
            ) != 0
        }
        .then(|| {
            let windows = unsafe {
                std::slice::from_raw_parts(children, num_children as usize)
                    .to_vec()
            };

            unsafe { xlib::XFree(children as *mut _) };

            windows
        })
    }

    fn set_active_window_border_color(&mut self, color_name: &str) {
        self.active_border_color = color::XftColor::new(
            self.display.clone(),
            self.screen,
            color_name.to_owned(),
        )
        .ok();
    }

    fn set_inactive_window_border_color(&mut self, color_name: &str) {
        self.inactive_border_color = color::XftColor::new(
            self.display.clone(),
            self.screen,
            color_name.to_owned(),
        )
        .ok();
    }
}

#[allow(dead_code)]
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

pub mod xpointer {
    use std::{
        ops::{Deref, DerefMut},
        ptr::{null, NonNull},
    };

    use x11::xlib::XFree;

    #[repr(C)]
    #[derive(Debug)]
    pub struct XPointer<T>(NonNull<T>);

    impl<T> XPointer<T> {
        pub fn build_with<F>(cb: F) -> Option<Self>
        where
            F: FnOnce(&mut *const ()),
        {
            let mut ptr = null() as *const ();
            cb(&mut ptr);
            NonNull::new(ptr as *mut T).map(|ptr| Self(ptr))
        }

        pub fn build_with_result<F, R>(cb: F) -> (Option<Self>, R)
        where
            F: FnOnce(&mut *const ()) -> R,
        {
            let mut ptr = null() as *const ();
            let result = cb(&mut ptr);
            (NonNull::new(ptr as *mut T).map(|ptr| Self(ptr)), result)
        }
    }

    impl<T> AsRef<T> for XPointer<T> {
        fn as_ref(&self) -> &T {
            &**self
        }
    }

    impl<T> AsMut<T> for XPointer<T> {
        fn as_mut(&mut self) -> &mut T {
            &mut **self
        }
    }

    impl<T> PartialEq for XPointer<T>
    where
        T: PartialEq,
    {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }

    impl<T> Eq for XPointer<T> where T: Eq {}

    impl<T> Deref for XPointer<T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            unsafe { self.0.as_ref() }
        }
    }

    impl<T> DerefMut for XPointer<T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            unsafe { self.0.as_mut() }
        }
    }

    impl<T> Drop for XPointer<T> {
        fn drop(&mut self) {
            unsafe { XFree(self.0.as_ptr() as _) };
        }
    }
}
