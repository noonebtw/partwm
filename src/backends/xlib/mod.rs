#![allow(unused_variables, dead_code)]
use log::{debug, error, warn};
use std::{ffi::CString, rc::Rc};

use thiserror::Error;

use x11::xlib::{
    self, Atom, KeyPress, KeyRelease, Window, XEvent, XInternAtom, XKeyEvent,
};

use crate::backends::{
    keycodes::KeyOrButton, window_event::ModifierKey,
    xlib::keysym::mouse_button_to_xbutton,
};

use self::keysym::{
    keysym_to_virtual_keycode, virtual_keycode_to_keysym, xev_to_mouse_button,
    XKeySym,
};

use super::{
    keycodes::VirtualKeyCode,
    window_event::{
        ButtonEvent, ConfigureEvent, DestroyEvent, EnterEvent, KeyEvent,
        KeyOrMouseBind, KeyState, MapEvent, ModifierState, MotionEvent, Point,
        UnmapEvent, WindowEvent,
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
    keybinds: Vec<KeyOrMouseBind>,
}

impl Drop for XLib {
    fn drop(&mut self) {
        self.close_dpy();
    }
}

impl XLib {
    fn new() -> Self {
        let (display, screen, root) = {
            let display = unsafe { xlib::XOpenDisplay(std::ptr::null()) };
            assert_ne!(display, std::ptr::null_mut());
            let screen = unsafe { xlib::XDefaultScreen(display) };
            let root = unsafe { xlib::XRootWindow(display, screen) };

            let display = Display::new(display);

            (display, screen, root)
        };

        let atoms = XLibAtoms::init(display.clone());

        Self {
            display,
            screen,
            root,
            modifier_state: ModifierState::empty(),
            atoms,
            keybinds: Vec::new(),
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
                KeyPress => self.modifier_state.insert_mod(modifier),
                KeyRelease => self.modifier_state.unset_mod(modifier),
                _ => unreachable!("keyyevent != (KeyPress | KeyRelease)"),
            }
        }
    }

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
        let bits = self.bits();
        let mut mask = 0;
        let numlock_mask = xlib
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

            // TODO: make painting the window border a seperate function, and configurable
            let screen = xlib::XDefaultScreenOfDisplay(self.dpy()).as_ref();

            if let Some(screen) = screen {
                xlib::XSetWindowBorder(self.dpy(), window, screen.white_pixel);
            }

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
            let screen = xlib::XDefaultScreenOfDisplay(self.dpy()).as_ref();

            if let Some(screen) = screen {
                xlib::XSetWindowBorder(self.dpy(), window, screen.black_pixel);
            }

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
        let screen_size = self.screen_size();
        self.move_window(window, screen_size);
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
        new_size: Option<super::window_event::Point<i32>>,
        new_pos: Option<super::window_event::Point<i32>>,
    ) {
        let position = new_pos.unwrap_or(Point::new(0, 0));
        let size = new_size.unwrap_or(Point::new(0, 0));
        let mut wc = xlib::XWindowChanges {
            x: position.x,
            y: position.y,
            width: size.x,
            height: size.y,
            border_width: 0,
            sibling: 0,
            stack_mode: 0,
        };

        let mask = {
            let mut mask = 0;
            if new_pos.is_some() {
                mask |= xlib::CWX | xlib::CWY;
            }
            if new_size.is_some() {
                mask |= xlib::CWWidth | xlib::CWHeight;
            }

            u32::from(mask)
        };

        unsafe {
            xlib::XConfigureWindow(self.dpy(), window, mask, &mut wc);
        }
    }

    fn screen_size(&self) -> Point<i32> {
        unsafe {
            let mut wa =
                std::mem::MaybeUninit::<xlib::XWindowAttributes>::zeroed();

            xlib::XGetWindowAttributes(self.dpy(), self.root, wa.as_mut_ptr());

            let wa = wa.assume_init();

            (wa.width, wa.height).into()
        }
    }

    fn get_window_size(&self, window: Self::Window) -> Option<Point<i32>> {
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
