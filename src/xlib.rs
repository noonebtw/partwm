use log::info;
use std::ptr::{null, null_mut};
use std::{ffi::CString, rc::Rc};

use x11::xlib::{
    self, AnyButton, AnyKey, AnyModifier, Atom, ButtonPressMask,
    ButtonReleaseMask, CWEventMask, ControlMask, CurrentTime, EnterWindowMask,
    FocusChangeMask, LockMask, Mod1Mask, Mod2Mask, Mod3Mask, Mod4Mask,
    Mod5Mask, PointerMotionMask, PropertyChangeMask, ShiftMask,
    StructureNotifyMask, SubstructureNotifyMask, SubstructureRedirectMask,
    Window, XCloseDisplay, XConfigureRequestEvent, XDefaultScreen, XEvent,
    XGetTransientForHint, XGrabPointer, XInternAtom, XKillClient, XMapWindow,
    XOpenDisplay, XRaiseWindow, XRootWindow, XSetErrorHandler, XSync,
    XUngrabButton, XUngrabKey, XUngrabPointer, XWarpPointer, XWindowAttributes,
};
use xlib::GrabModeAsync;

use log::error;

use crate::clients::Client;

pub struct XLib {
    display: Display,
    root: Window,
    _screen: i32,
    atoms: Atoms,
    global_keybinds: Vec<KeyOrButton>,
}

struct Atoms {
    protocols: Atom,
    delete_window: Atom,
    active_window: Atom,
    take_focus: Atom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyOrButton {
    Key(i32, u32),
    Button(u32, u32, u64),
}

impl KeyOrButton {
    #[allow(dead_code)]
    pub fn key(keycode: i32, modmask: u32) -> Self {
        Self::Key(keycode, modmask)
    }
    pub fn button(button: u32, modmask: u32, buttonmask: i64) -> Self {
        Self::Button(button, modmask, buttonmask as u64)
    }
}

#[derive(Clone)]
pub struct Display(Rc<*mut xlib::Display>);

impl Drop for XLib {
    fn drop(&mut self) {
        self.close_dpy();
    }
}

impl XLib {
    pub fn new() -> Self {
        let (display, _screen, root) = unsafe {
            let display = XOpenDisplay(null());

            assert_ne!(display, null_mut());

            let display = Display::new(display);
            let screen = XDefaultScreen(display.get());
            let root = XRootWindow(display.get(), screen);

            (display, screen, root)
        };

        Self {
            atoms: Atoms::init(display.clone()),
            global_keybinds: Vec::new(),
            root,
            _screen,
            display,
        }
    }

    pub fn init(&mut self) {
        unsafe {
            let mut window_attributes =
                std::mem::MaybeUninit::<xlib::XSetWindowAttributes>::zeroed()
                    .assume_init();

            window_attributes.event_mask = SubstructureRedirectMask
                | StructureNotifyMask
                | SubstructureNotifyMask
                | EnterWindowMask
                | PointerMotionMask
                | ButtonPressMask;

            xlib::XChangeWindowAttributes(
                self.dpy(),
                self.root,
                CWEventMask,
                &mut window_attributes,
            );

            xlib::XSelectInput(
                self.dpy(),
                self.root,
                window_attributes.event_mask,
            );

            XSetErrorHandler(Some(xlib_error_handler));

            XSync(self.dpy(), 0);
        }

        self.grab_global_keybinds(self.root);
    }

    pub fn add_global_keybind(&mut self, key: KeyOrButton) {
        self.global_keybinds.push(key);
    }

    #[allow(dead_code)]
    fn ungrab_global_keybings(&self, window: Window) {
        unsafe {
            XUngrabButton(self.dpy(), AnyButton as u32, AnyModifier, window);
            XUngrabKey(self.dpy(), AnyKey, AnyModifier, window);
        }
    }

    fn grab_global_keybinds(&self, window: Window) {
        for kb in self.global_keybinds.iter() {
            self.grab_key_or_button(window, kb);
        }
    }

    #[allow(dead_code)]
    pub fn remove_global_keybind(&mut self, key: &KeyOrButton) {
        if self.global_keybinds.contains(key) {
            self.global_keybinds.retain(|kb| kb != key);
        }
    }

    fn dpy(&self) -> *mut xlib::Display {
        self.display.get()
    }

    #[allow(dead_code)]
    pub fn squash_event(&self, event_type: i32) -> XEvent {
        unsafe {
            let mut event =
                std::mem::MaybeUninit::<xlib::XEvent>::zeroed().assume_init();

            while xlib::XCheckTypedEvent(self.dpy(), event_type, &mut event)
                != 0
            {}

            event
        }
    }

    pub fn next_event(&self) -> XEvent {
        unsafe {
            let mut event =
                std::mem::MaybeUninit::<xlib::XEvent>::zeroed().assume_init();
            xlib::XNextEvent(self.dpy(), &mut event);

            event
        }
    }

    pub fn grab_key_or_button(&self, window: Window, key: &KeyOrButton) {
        let numlock_mask = self.get_numlock_mask();
        let modifiers =
            vec![0, LockMask, numlock_mask, LockMask | numlock_mask];

        for modifier in modifiers.iter() {
            match key {
                &KeyOrButton::Key(keycode, modmask) => {
                    unsafe {
                        xlib::XGrabKey(
                            self.dpy(),
                            keycode,
                            modmask | modifier,
                            window,
                            1, /* true */
                            GrabModeAsync,
                            GrabModeAsync,
                        );
                    }
                }
                &KeyOrButton::Button(button, modmask, buttonmask) => {
                    unsafe {
                        xlib::XGrabButton(
                            self.dpy(),
                            button,
                            modmask | modifier,
                            window,
                            1, /*true */
                            buttonmask as u32,
                            GrabModeAsync,
                            GrabModeAsync,
                            0,
                            0,
                        );
                    }
                }
            }
        }
    }

    pub fn focus_client(&self, client: &Client) {
        unsafe {
            xlib::XSetInputFocus(
                self.dpy(),
                client.window,
                xlib::RevertToPointerRoot,
                xlib::CurrentTime,
            );

            let screen = xlib::XDefaultScreenOfDisplay(self.dpy()).as_ref();

            if let Some(screen) = screen {
                xlib::XSetWindowBorder(
                    self.dpy(),
                    client.window,
                    screen.white_pixel,
                );
            }

            xlib::XChangeProperty(
                self.dpy(),
                self.root,
                self.atoms.active_window,
                xlib::XA_WINDOW,
                32,
                xlib::PropModeReplace,
                &client.window as *const u64 as *const _,
                1,
            );
        }

        self.send_protocol(client, self.atoms.take_focus);
    }

    pub fn unfocus_client(&self, client: &Client) {
        //info!("unfocusing client: {:?}", client);

        unsafe {
            xlib::XSetInputFocus(
                self.dpy(),
                self.root,
                xlib::RevertToPointerRoot,
                xlib::CurrentTime,
            );

            let screen = xlib::XDefaultScreenOfDisplay(self.dpy()).as_ref();

            if let Some(screen) = screen {
                xlib::XSetWindowBorder(
                    self.dpy(),
                    client.window,
                    screen.black_pixel,
                );
            }

            // xlib::XDeleteProperty(
            //     self.dpy(),
            //     self.root,
            //     self.atoms.active_window,
            // );
        }
    }

    pub fn move_resize_client(&self, client: &Client) {
        let mut windowchanges = xlib::XWindowChanges {
            x: client.position.0,
            y: client.position.1,
            width: client.size.0,
            height: client.size.1,
            border_width: 0,
            sibling: 0,
            stack_mode: 0,
        };

        if client.size.0 < 1 || client.size.1 < 1 {
            error!("client {:?} size is less than 1 pixel!", client);
        } else {
            unsafe {
                xlib::XConfigureWindow(
                    self.dpy(),
                    client.window,
                    (xlib::CWY | xlib::CWX | xlib::CWHeight | xlib::CWWidth)
                        as u32,
                    &mut windowchanges,
                );

                // I don't think I have to call this ~
                //self.configure_client(client);
            }
        }
    }

    pub fn move_client(&self, client: &Client) {
        let mut wc = xlib::XWindowChanges {
            x: client.position.0,
            y: client.position.1,
            width: client.size.0,
            height: client.size.1,
            border_width: 0,
            sibling: 0,
            stack_mode: 0,
        };

        if client.size.0 < 1 || client.size.1 < 1 {
            error!("client {:?} size is less than 1 pixel!", client);
        } else {
            unsafe {
                xlib::XConfigureWindow(
                    self.dpy(),
                    client.window,
                    (xlib::CWX | xlib::CWY) as u32,
                    &mut wc,
                );
            }
        }
    }

    pub fn resize_client(&self, client: &Client) {
        let mut wc = xlib::XWindowChanges {
            x: client.position.0,
            y: client.position.1,
            width: client.size.0,
            height: client.size.1,
            border_width: 0,
            sibling: 0,
            stack_mode: 0,
        };

        if client.size.0 < 1 || client.size.1 < 1 {
            error!("client {:?} size is less than 1 pixel!", client);
        } else {
            unsafe {
                xlib::XConfigureWindow(
                    self.dpy(),
                    client.window,
                    (xlib::CWWidth | xlib::CWHeight) as u32,
                    &mut wc,
                );
            }
        }
    }

    pub fn hide_client(&self, client: &Client) {
        let mut wc = xlib::XWindowChanges {
            x: client.size.0 * -2,
            y: client.position.1,
            width: client.size.0,
            height: client.size.1,
            border_width: 0,
            sibling: 0,
            stack_mode: 0,
        };

        if client.size.0 < 1 || client.size.1 < 1 {
            error!("client {:?} size is less than 1 pixel!", client);
        } else {
            unsafe {
                xlib::XConfigureWindow(
                    self.dpy(),
                    client.window,
                    (xlib::CWX | xlib::CWY) as u32,
                    &mut wc,
                );
            }
        }
    }

    pub fn raise_client(&self, client: &Client) {
        unsafe {
            XRaiseWindow(self.dpy(), client.window);
        }
    }

    pub fn get_window_size(&self, window: Window) -> Option<(i32, i32)> {
        let mut wa = unsafe {
            std::mem::MaybeUninit::<xlib::XWindowAttributes>::zeroed()
                .assume_init()
        };

        if unsafe {
            xlib::XGetWindowAttributes(self.dpy(), window, &mut wa) != 0
        } {
            Some((wa.width, wa.height))
        } else {
            None
        }
    }

    #[allow(dead_code)]
    fn get_window_attributes(
        &self,
        window: Window,
    ) -> Option<XWindowAttributes> {
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

    pub fn get_transient_for_window(&self, window: Window) -> Option<Window> {
        let mut transient_for: Window = 0;

        if unsafe {
            XGetTransientForHint(self.dpy(), window, &mut transient_for) != 0
        } {
            Some(transient_for)
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn expose_client(&self, client: &Client) {
        self.expose_window(client.window);
    }

    #[allow(dead_code)]
    fn expose_window(&self, window: Window) {
        if let Some(wa) = self.get_window_attributes(window) {
            unsafe {
                xlib::XClearArea(
                    self.dpy(),
                    window,
                    0,
                    0,
                    wa.width as u32,
                    wa.height as u32,
                    1,
                );
            }
        }
    }

    pub fn configure_window(&self, event: &XConfigureRequestEvent) {
        let mut wc = xlib::XWindowChanges {
            x: event.x,
            y: event.y,
            width: event.width,
            height: event.height,
            border_width: event.border_width,
            sibling: event.above,
            stack_mode: event.detail,
        };

        unsafe {
            xlib::XConfigureWindow(
                self.dpy(),
                event.window,
                event.value_mask as u32,
                &mut wc,
            );
        }
    }

    pub fn configure_client(&self, client: &Client, border: i32) {
        let mut event = xlib::XConfigureEvent {
            type_: xlib::ConfigureNotify,
            display: self.dpy(),
            event: client.window,
            window: client.window,
            x: client.position.0,
            y: client.position.1,
            width: client.size.0,
            height: client.size.1,
            border_width: border,
            override_redirect: 0,
            send_event: 0,
            serial: 0,
            above: 0,
        };

        unsafe {
            xlib::XSetWindowBorderWidth(
                self.dpy(),
                event.window,
                event.border_width as u32,
            );

            xlib::XSendEvent(
                self.dpy(),
                event.window,
                0,
                StructureNotifyMask,
                &mut event as *mut _ as *mut XEvent,
            );
        }
    }

    pub fn map_window(&self, window: Window) {
        unsafe {
            XMapWindow(self.dpy(), window);

            xlib::XSelectInput(
                self.dpy(),
                window,
                EnterWindowMask
                    | FocusChangeMask
                    | PropertyChangeMask
                    | StructureNotifyMask,
            );
        }

        self.grab_global_keybinds(window);
    }

    pub fn dimensions(&self) -> (i32, i32) {
        unsafe {
            let mut wa =
                std::mem::MaybeUninit::<xlib::XWindowAttributes>::zeroed()
                    .assume_init();

            xlib::XGetWindowAttributes(self.dpy(), self.root, &mut wa);

            info!("Root window dimensions: {}, {}", wa.width, wa.height);

            (wa.width, wa.height)
        }
    }

    fn close_dpy(&self) {
        unsafe {
            XCloseDisplay(self.dpy());
        }
    }

    pub fn kill_client(&self, client: &Client) {
        if !self.send_protocol(client, self.atoms.delete_window) {
            unsafe {
                XKillClient(self.dpy(), client.window);
            }
        }
    }

    pub fn grab_cursor(&self) {
        unsafe {
            XGrabPointer(
                self.dpy(),
                self.root,
                0,
                (ButtonPressMask | ButtonReleaseMask | PointerMotionMask)
                    as u32,
                GrabModeAsync,
                GrabModeAsync,
                0,
                0,
                CurrentTime,
            );
        }
    }

    pub fn release_cursor(&self) {
        unsafe {
            XUngrabPointer(self.dpy(), CurrentTime);
        }
    }

    pub fn move_cursor(&self, window: Option<Window>, position: (i32, i32)) {
        unsafe {
            XWarpPointer(
                self.dpy(),
                0,
                window.unwrap_or(self.root),
                0,
                0,
                0,
                0,
                position.0,
                position.1,
            );
        }
    }

    fn check_for_protocol(&self, client: &Client, proto: xlib::Atom) -> bool {
        let mut protos: *mut xlib::Atom = null_mut();
        let mut num_protos: i32 = 0;

        unsafe {
            if xlib::XGetWMProtocols(
                self.dpy(),
                client.window,
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

    fn send_protocol(&self, client: &Client, proto: xlib::Atom) -> bool {
        if self.check_for_protocol(client, proto) {
            let mut data = xlib::ClientMessageData::default();
            data.set_long(0, proto as i64);

            let mut event = XEvent {
                client_message: xlib::XClientMessageEvent {
                    type_: xlib::ClientMessage,
                    serial: 0,
                    display: self.dpy(),
                    send_event: 0,
                    window: client.window,
                    format: 32,
                    message_type: self.atoms.protocols,
                    data,
                },
            };

            unsafe {
                xlib::XSendEvent(
                    self.dpy(),
                    client.window,
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

    pub fn make_key<S>(&self, key: S, modmask: u32) -> KeyOrButton
    where
        S: AsRef<str>,
    {
        let key = self.keycode(key);

        KeyOrButton::Key(key, modmask)
    }

    fn keycode<S>(&self, string: S) -> i32
    where
        S: AsRef<str>,
    {
        let c_string = CString::new(string.as_ref()).unwrap();

        unsafe {
            let keysym = xlib::XStringToKeysym(c_string.as_ptr());
            xlib::XKeysymToKeycode(self.dpy(), keysym) as i32
        }
    }

    fn get_numlock_mask(&self) -> u32 {
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
                        return 1 << i;
                    }
                }
            }
        }

        0
    }

    pub fn get_clean_mask(&self) -> u32 {
        !(self.get_numlock_mask() | LockMask)
            & (ShiftMask
                | ControlMask
                | Mod1Mask
                | Mod2Mask
                | Mod3Mask
                | Mod4Mask
                | Mod5Mask)
    }

    #[allow(dead_code)]
    pub fn clean_mask(&self, mask: u32) -> u32 {
        mask & self.get_clean_mask()
    }

    pub fn are_masks_equal(&self, rhs: u32, lhs: u32) -> bool {
        let clean = self.get_clean_mask();

        rhs & clean == lhs & clean
    }
}

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

impl Atoms {
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
