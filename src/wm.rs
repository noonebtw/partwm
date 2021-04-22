/*
use std::{
    borrow::Borrow,
    cell::RefCell,
    collections::HashSet,
    ffi::CString,
    hash::{Hash, Hasher},
    io::{Error, ErrorKind, Result},
    ops::Deref,
    ptr::{null, null_mut},
    rc::{Rc, Weak},
};

use x11::{
    xlib,
    xlib::{
        Atom, ButtonPressMask, ButtonReleaseMask, CWEventMask, ControlMask, EnterWindowMask,
        FocusChangeMask, GrabModeAsync, LockMask, Mod1Mask, Mod2Mask, Mod3Mask, Mod4Mask, Mod5Mask,
        PointerMotionMask, PropertyChangeMask, ShiftMask, StructureNotifyMask,
        SubstructureNotifyMask, SubstructureRedirectMask, Window, XDefaultScreen, XEvent,
        XInternAtom, XOpenDisplay, XRootWindow,
    },
};

use log::info;

use nix::unistd::{close, execvp, fork, setsid, ForkResult};

#[derive(Clone)]
pub struct Display(Rc<*mut x11::xlib::Display>);

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

pub struct WMAtoms {
    protocols: Atom,
    delete_window: Atom,
    active_window: Atom,
    take_focus: Atom,
}

impl WMAtoms {
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

use crate::util::BuildIdentityHasher;
use weak_table::WeakHashSet;

#[derive(Clone, Debug)]
struct VirtualScreen {
    master_stack: WeakHashSet<Weak<Client>, BuildIdentityHasher>,
    aux_stack: WeakHashSet<Weak<Client>, BuildIdentityHasher>,
    focused_client: Weak<Client>,
}

impl VirtualScreen {
    fn new() -> Self {
        Self {
            master_stack: Default::default(),
            aux_stack: Default::default(),
            focused_client: Weak::new(),
        }
    }

    fn contains_client(&self, client: &Rc<Client>) -> bool {
        self.master_stack.contains(client.as_ref()) || self.aux_stack.contains(client.as_ref())
    }
}

pub struct XLibState {
    display: Display,
    root: Window,
    screen: i32,
    pub atoms: WMAtoms,
}

impl XLibState {
    pub fn new() -> Self {
        let (display, screen, root) = unsafe {
            let display = XOpenDisplay(null());
            assert_ne!(display, null_mut());

            let display = Display::new(display);
            let screen = XDefaultScreen(display.get());
            let root = XRootWindow(display.get(), screen);

            (display, screen, root)
        };

        Self {
            atoms: WMAtoms::init(display.clone()),
            display,
            root,
            screen,
        }
    }

    pub fn dpy(&self) -> *mut x11::xlib::Display {
        self.display.get()
    }

    pub fn root(&self) -> Window {
        self.root
    }

    pub fn screen(&self) -> i32 {
        self.screen
    }

    pub fn grab_key(&self, window: xlib::Window, keycode: i32, mod_mask: u32) {
        let numlock_mask = self.numlock_mask();
        let modifiers = vec![0, LockMask, numlock_mask, LockMask | numlock_mask];
        for &modifier in modifiers.iter() {
            unsafe {
                xlib::XGrabKey(
                    self.dpy(),
                    keycode,
                    mod_mask | modifier,
                    window,
                    1, /* true */
                    GrabModeAsync,
                    GrabModeAsync,
                );
            }
        }
    }

    pub fn grab_button(&self, window: xlib::Window, button: u32, mod_mask: u32, button_mask: i64) {
        let numlock_mask = self.numlock_mask();
        let modifiers = vec![0, LockMask, numlock_mask, LockMask | numlock_mask];

        modifiers.iter().for_each(|&modifier| {
            unsafe {
                xlib::XGrabButton(
                    self.dpy(),
                    button,
                    mod_mask | modifier,
                    window,
                    1, /*true */
                    button_mask as u32,
                    GrabModeAsync,
                    GrabModeAsync,
                    0,
                    0,
                );
            }
        });
    }

    pub fn keycode<S>(&self, string: S) -> i32
    where
        S: Into<String>,
    {
        let c_string = CString::new(string.into()).unwrap();
        unsafe {
            let keysym = xlib::XStringToKeysym(c_string.as_ptr());
            xlib::XKeysymToKeycode(self.dpy(), keysym) as i32
        }
    }

    fn check_for_protocol(&self, window: xlib::Window, proto: xlib::Atom) -> bool {
        let mut protos: *mut xlib::Atom = null_mut();
        let mut num_protos: i32 = 0;

        unsafe {
            if xlib::XGetWMProtocols(self.dpy(), window, &mut protos, &mut num_protos) != 0 {
                for i in 0..num_protos {
                    if *protos.offset(i as isize) == proto {
                        return true;
                    }
                }
            }
        }

        return false;
    }

    fn send_event(&self, window: xlib::Window, proto: xlib::Atom) -> bool {
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
                    message_type: self.atoms.protocols,
                    data,
                },
            };

            unsafe {
                xlib::XSendEvent(self.dpy(), window, 0, xlib::NoEventMask, &mut event);
            }

            true
        } else {
            false
        }
    }

    fn numlock_mask(&self) -> u32 {
        unsafe {
            let modmap = xlib::XGetModifierMapping(self.dpy());
            let max_keypermod = (*modmap).max_keypermod;

            for i in 0..8 {
                for j in 0..max_keypermod {
                    if *(*modmap)
                        .modifiermap
                        .offset((i * max_keypermod + j) as isize)
                        == xlib::XKeysymToKeycode(self.dpy(), x11::keysym::XK_Num_Lock as u64)
                    {
                        return 1 << i;
                    }
                }
            }
        }

        0
    }

    fn clean_mask(&self) -> u32 {
        !(self.numlock_mask() | LockMask)
            & (ShiftMask | ControlMask | Mod1Mask | Mod2Mask | Mod3Mask | Mod4Mask | Mod5Mask)
    }
}

struct Key {
    keycode: i32,
    mod_mask: u32,
}

struct Button {
    button: i32,
    mod_mask: u32,
    button_mask: u64,
}

enum KeyOrButton {
    Key(Key),
    Button(Button),
}

pub struct WMState {
    xlib_state: XLibState,
    key_handlers: Vec<(i32, u32, Rc<dyn Fn(&mut Self, &XEvent)>)>,
    // (button, mod_mask, button_mask)
    buttons: Vec<(u32, u32, i64)>,
    event_handlers: Vec<Rc<dyn Fn(&mut Self, &XEvent)>>,

    // MutState:

    //move_window:
    // u64 : window to move
    // (i32, i32) : initial cursor position
    // (i32, i32) : initial window position
    move_window: Option<(u64, (i32, i32), (i32, i32))>,
    //resize_window:
    // u64 : window to move
    // (i32, i32) : initial window position
    resize_window: Option<(u64, (i32, i32))>,
    clients: HashSet<Rc<Client>, BuildIdentityHasher>,
    focused_client: Weak<Client>,
    current_vscreen: usize,
    virtual_screens: Vec<VirtualScreen>,
}

impl WMState {
    pub fn new() -> Self {
        Self {
            xlib_state: XLibState::new(),
            key_handlers: vec![],
            event_handlers: vec![],
            buttons: vec![],
            move_window: None,
            resize_window: None,
            clients: Default::default(),
            focused_client: Weak::new(),
            current_vscreen: 0,
            virtual_screens: vec![VirtualScreen::new()],
        }
    }

    pub fn init() -> Self {
        let state = Self::new()
            .grab_button(
                1,
                Mod1Mask,
                ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
            )
            .grab_button(
                2,
                Mod1Mask,
                ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
            )
            .grab_button(
                3,
                Mod1Mask,
                ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
            )
            .add_event_handler(Self::handle_toggle_floating)
            .add_event_handler(Self::handle_move_window)
            .add_event_handler(Self::handle_resize_window)
            .add_key_handler("T", Mod1Mask, |state, _| {
                println!("spawning terminal");
                let _ = state.spawn("xterm", &[]);
            })
            .add_key_handler("Left", Mod1Mask, |state, _| {
                state.handle_change_virtual_screen(-1);
            })
            .add_key_handler("Right", Mod1Mask, |state, _| {
                state.handle_change_virtual_screen(1);
            })
            .add_key_handler("M", Mod1Mask, Self::handle_switch_stack)
            .add_key_handler("Q", Mod1Mask, |state, event| unsafe {
                if event.key.subwindow != 0 {
                    if !state
                        .xlib_state
                        .send_event(event.key.subwindow, state.xlib_state.atoms.delete_window)
                    {
                        xlib::XKillClient(state.dpy(), event.key.subwindow);
                    }
                }
            })
            .add_key_handler("Q", Mod1Mask | ShiftMask, |state, _event| {
                unsafe {
                    xlib::XCloseDisplay(state.dpy());
                }

                std::process::exit(0);
            });

        unsafe {
            let mut wa: xlib::XSetWindowAttributes = std::mem::MaybeUninit::zeroed().assume_init();
            wa.event_mask = SubstructureRedirectMask
                | StructureNotifyMask
                | SubstructureNotifyMask
                | EnterWindowMask
                | PointerMotionMask
                | ButtonPressMask;

            xlib::XChangeWindowAttributes(state.dpy(), state.root(), CWEventMask, &mut wa);

            xlib::XSelectInput(state.dpy(), state.root(), wa.event_mask);
        }

        state
    }

    // MutState Functions:

    fn stack_unstacked_clients(&mut self) {
        info!("[stack_unstacked_clients] ");

        let unstacked_clients = self
            .clients
            .iter()
            .filter(|&c| !c.floating && !self.is_client_stacked(c))
            .cloned()
            .collect::<Vec<_>>();

        unstacked_clients.iter().for_each(|c| {
            info!(
                "[stack_unstacked_clients] inserting Client({:?}) into aux_stack",
                c
            );

            self.virtual_screens[self.current_vscreen]
                .aux_stack
                .insert(c.clone());
        });
    }

    fn is_client_stacked(&self, client: &Rc<Client>) -> bool {
        self.virtual_screens
            .iter()
            .any(|vs| vs.contains_client(client))
    }

    fn client_for_window(&self, window: Window) -> Option<Rc<Client>> {
        self.clients
            .iter()
            .filter(|&c| c.window == window)
            .next()
            .cloned()
    }

    fn switch_stack_for_client(&mut self, client: Rc<Client>) {
        info!("[switch_stack_for_client] client: {:#?}", client);

        if self.virtual_screens[self.current_vscreen]
            .master_stack
            .contains(client.as_ref())
        {
            self.virtual_screens[self.current_vscreen]
                .master_stack
                .remove(client.as_ref());
            self.virtual_screens[self.current_vscreen]
                .aux_stack
                .insert(client.clone());
            info!("[switch_stack_for_client] moved to aux stack");
        } else {
            self.virtual_screens[self.current_vscreen]
                .aux_stack
                .remove(client.as_ref());
            self.virtual_screens[self.current_vscreen]
                .master_stack
                .insert(client.clone());
            info!("[switch_stack_for_client] moved to master stack");
        }

        self.clients.replace(Client::new_rc(InnerClient {
            floating: false,
            ..**client.as_ref()
        }));
    }

    fn refresh_screen(&mut self) {
        let current_vscreen = self.current_vscreen;

        self.stack_unstacked_clients();

        if let Some(vs) = self.virtual_screens.get_mut(self.current_vscreen) {
            vs.master_stack.retain(|c| !c.0.borrow().floating);
            vs.aux_stack.retain(|c| !c.floating);

            if vs.master_stack.is_empty() {
                info!("[refresh_screen] master stack was empty, pushing first client if exists:");
                vs.aux_stack.iter().filter(|c| !c.floating).next().map(|c| {
                    info!("[arrange_clients] Client({:#?})", c);

                    self.virtual_screens[current_vscreen]
                        .master_stack
                        .insert(c.clone());
                    self.virtual_screens[current_vscreen]
                        .aux_stack
                        .remove(c.as_ref());
                });
            }
        }
    }

    pub fn run(mut self) -> Self {
        let event_handlers = self.event_handlers.clone();
        let key_handlers = self.key_handlers.clone();

        loop {
            let event = unsafe {
                let mut event: xlib::XEvent = std::mem::MaybeUninit::zeroed().assume_init();
                xlib::XNextEvent(self.dpy(), &mut event);

                event
            };

            for handler in event_handlers.iter() {
                handler(&mut self, &event);
            }

            match event.get_type() {
                xlib::MapRequest => {
                    self.map_request(unsafe { &event.map_request });
                }
                xlib::UnmapNotify => {
                    self.unmap_notify(unsafe { &event.unmap });
                }
                xlib::DestroyNotify => {
                    self.destroy_notify(unsafe { &event.destroy_window });
                }
                xlib::ConfigureRequest => {
                    self.configure_request(unsafe { &event.configure_request });
                }
                xlib::EnterNotify => {
                    self.enter_notify(unsafe { &event.crossing });
                }
                xlib::ButtonPress => {
                    self.button_press(unsafe { &event.button });
                }
                xlib::KeyPress => {
                    let clean_mask = self.xlib_state.clean_mask();

                    key_handlers.iter().for_each(|(key, mask, handler)| {
                        if unsafe {
                            event.key.keycode == *key as u32
                                && event.key.state & clean_mask == *mask & clean_mask
                        } {
                            handler(&mut self, &event);
                        }
                    })
                }
                _ => {}
            }
        }
    }

    pub fn dpy(&self) -> *mut xlib::Display {
        self.xlib_state.dpy()
    }

    pub fn root(&self) -> xlib::Window {
        self.xlib_state.root()
    }

    pub fn grab_button(mut self, button: u32, mod_mask: u32, button_mask: i64) -> Self {
        self.buttons.push((button, mod_mask, button_mask));
        self.xlib_state
            .grab_button(self.root(), button, mod_mask, button_mask);

        self
    }

    #[allow(dead_code)]
    pub fn add_event_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Self, &XEvent) + 'static,
    {
        self.event_handlers.push(Rc::new(handler));

        self
    }

    pub fn add_key_handler<S, F>(mut self, key: S, mask: u32, handler: F) -> Self
    where
        S: Into<String>,
        F: Fn(&mut Self, &XEvent) + 'static,
    {
        let keycode = self.xlib_state.keycode(key);

        self.key_handlers.push((keycode, mask, Rc::new(handler)));
        self.xlib_state.grab_key(self.root(), keycode, mask);

        self
    }

    fn map_request(&mut self, event: &xlib::XMapRequestEvent) {
        info!("[MapRequest] event: {:#?}", event);

        if self.client_for_window(event.window).is_none() {
            info!("[MapRequest] new client: {:#?}", event.window);
            let client = Rc::new(Client::new(event.window));
            self.clients.insert(client.clone());

            unsafe {
                xlib::XMapWindow(self.dpy(), client.window);

                xlib::XSelectInput(
                    self.dpy(),
                    event.window,
                    EnterWindowMask | FocusChangeMask | PropertyChangeMask | StructureNotifyMask,
                );
            }

            self.buttons
                .iter()
                .for_each(|&(button, mod_mask, button_mask)| {
                    self.xlib_state
                        .grab_button(client.window, button, mod_mask, button_mask);
                });

            self.arrange_clients();
            self.focus_client(&client);
        }
    }

    fn unmap_notify(&mut self, event: &xlib::XUnmapEvent) {
        info!("[UnmapNotify] event: {:#?}", event);

        if event.send_event == 0 {
            if let Some(client) = self.client_for_window(event.window) {
                self.clients.remove(&client);
                info!("[UnmapNotify] removing client: {:#?}", client);
            }
        }

        self.arrange_clients();
    }

    fn destroy_notify(&mut self, event: &xlib::XDestroyWindowEvent) {
        info!("[DestroyNotify] event: {:?}", event);

        if let Some(client) = self.client_for_window(event.window) {
            self.clients.remove(&client);

            info!("[DestroyNotify] removed entry: {:?}", client);
        }

        self.arrange_clients();
    }

    fn configure_request(&mut self, event: &xlib::XConfigureRequestEvent) {
        info!("[ConfigureRequest] event: {:?}", event);

        match self.client_for_window(event.window) {
            Some(client) => {
                info!("[ConfigureRequest] found Client {:#?}", client,);

                self.configure_client(&client);
            }
            _ => {
                info!(
                              "[ConfigureRequest] no client found for Window({:?}), calling XConfigureWindow()",
                              event.window);

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
        }
    }

    fn enter_notify(&mut self, event: &xlib::XCrossingEvent) {
        info!("[EnterNotify] event: {:?}", event);

        if let Some(client) = self.client_for_window(event.window) {
            info!("[EnterNotify] focusing Client ({:?})", client);
            self.focus_client(&client);
        }
    }

    fn button_press(&mut self, event: &xlib::XButtonEvent) {
        info!("[ButtonPress] event: {:?}", event);

        if let Some(client) = self.client_for_window(event.subwindow) {
            info!("[ButtonPress] focusing Client ({:?})", client);
            self.focus_client(&client);

            info!("[ButtonPress] raising Window({:?})", event.window);

            unsafe {
                xlib::XRaiseWindow(self.dpy(), client.window);
                xlib::XSync(self.dpy(), 0);
            }
        }
    }

    fn unfocus_client(&self, client: &Rc<Client>) {
        unsafe {
            xlib::XSetInputFocus(
                self.dpy(),
                client.window,
                xlib::RevertToPointerRoot,
                xlib::CurrentTime,
            );

            xlib::XDeleteProperty(self.dpy(), self.root(), self.xlib_state.atoms.active_window);
        }
    }

    fn focus_client(&mut self, client: &Rc<Client>) {
        if let Some(focused_client) = self.focused_client.upgrade() {
            self.unfocus_client(&focused_client);
        }

        self.focused_client = Rc::downgrade(client);

        self.virtual_screens[self.current_vscreen].focused_client = self.focused_client.clone();

        unsafe {
            xlib::XSetInputFocus(
                self.dpy(),
                client.window,
                xlib::RevertToPointerRoot,
                xlib::CurrentTime,
            );

            xlib::XChangeProperty(
                self.dpy(),
                self.root(),
                self.xlib_state.atoms.active_window,
                xlib::XA_WINDOW,
                32,
                xlib::PropModeReplace,
                &client.window as *const u64 as *const _,
                1,
            );
        }

        self.xlib_state
            .send_event(client.window, self.xlib_state.atoms.take_focus);
    }

    fn arrange_clients(&self) {
        let (screen_w, screen_h) = unsafe {
            (
                xlib::XDisplayWidth(self.dpy(), self.xlib_state.screen()),
                xlib::XDisplayHeight(self.dpy(), self.xlib_state.screen()),
            )
        };

        if !self.clients.is_empty() {
            info!("[arrange_clients] refreshing screen");
            self.refresh_screen();

            let window_w = {
                if self.virtual_screens[self.current_vscreen]
                    .aux_stack
                    .is_empty()
                {
                    screen_w
                } else {
                    screen_w / 2
                }
            };

            if let Some(vc) = self.virtual_screens.get_mut(self.current_vscreen) {
                vc.master_stack
                    .iter()
                    .zip(std::iter::repeat(vc.master_stack.len()))
                    .enumerate()
                    .chain(
                        vc.aux_stack
                            .iter()
                            .zip(std::iter::repeat(vc.aux_stack.len()))
                            .enumerate(),
                    )
                    .for_each(|(i, (client, length))| {
                        let (mut wc, size, position) = {
                            let window_h = screen_h / length as i32;

                            let size = (window_w, window_h);
                            let position = (0, window_h * i as i32);

                            (
                                xlib::XWindowChanges {
                                    x: position.0,
                                    y: position.1,
                                    width: size.0,
                                    height: size.1,
                                    border_width: 0,
                                    sibling: 0,
                                    stack_mode: 0,
                                },
                                size,
                                position,
                            )
                        };

                        unsafe {
                            xlib::XConfigureWindow(
                                self.dpy(),
                                client.window,
                                (xlib::CWY | xlib::CWX | xlib::CWHeight | xlib::CWWidth) as u32,
                                &mut wc,
                            );

                            self.clients.replace(Client::new_rc(InnerClient {
                                size,
                                position,
                                ..**client
                            }));

                            self.configure_client(&client);

                            xlib::XSync(self.dpy(), 0);
                        }
                    });
            }
        }
    }

    fn configure_client(&self, client: &Rc<Client>) {
        let mut event = {
            xlib::XConfigureEvent {
                type_: xlib::ConfigureNotify,
                display: self.dpy(),
                event: client.window,
                window: client.window,
                x: client.position.0,
                y: client.position.1,
                width: client.size.0,
                height: client.size.1,
                border_width: 0,
                override_redirect: 0,
                send_event: 0,
                serial: 0,
                above: 0,
            }
        };

        unsafe {
            xlib::XSendEvent(
                self.dpy(),
                event.window,
                0,
                StructureNotifyMask,
                &mut event as *mut _ as *mut XEvent,
            );
        }
    }

    fn handle_change_virtual_screen(&mut self, direction: i32) {
        assert!(direction == 1 || direction == -1);

        // hide all windows from current virtual screen
        self.virtual_screens[self.current_vscreen]
            .master_stack
            .iter()
            .chain(self.virtual_screens[self.current_vscreen].aux_stack.iter())
            .for_each(|c| unsafe {
                xlib::XMoveWindow(self.dpy(), c.window, c.size.0 * -2, c.position.1);
            });

        // change current_vscreen variable
        let mut new_vscreen = self.current_vscreen as isize + direction as isize;

        if new_vscreen >= self.virtual_screens.len() as isize {
            self.virtual_screens.push(VirtualScreen::new());
        } else if new_vscreen < 0 {
            new_vscreen = self.virtual_screens.len() as isize - 1;
        }

        self.current_vscreen = new_vscreen as usize;

        self.arrange_clients();

        // focus the focused cliend of the new virtual screen
        if let Some(client) = self.virtual_screens[self.current_vscreen]
            .focused_client
            .upgrade()
        {
            self.focus_client(&client);
        }
    }

    fn handle_switch_stack(&mut self, event: &XEvent) {
        if let Some(client) = self.client_for_window(event.button.subwindow) {
            self.switch_stack_for_client(client);
        }

        self.arrange_clients();
    }

    fn handle_toggle_floating(&mut self, event: &XEvent) {
        if event.get_type() == xlib::ButtonPress {
            let event = unsafe { event.button };
            let clean_mask = self.xlib_state.clean_mask();

            if event.button == 2
                && event.state & clean_mask == Mod1Mask & clean_mask
                && event.subwindow != 0
            {
                if let Some(client) = self.client_for_window(event.subwindow) {
                    info!(
                        "[handle_toggle_floating] {:#?} floating -> {:?}",
                        client, !client.floating
                    );

                    self.clients.replace(Client::new_rc(InnerClient {
                        floating: !client.floating,
                        ..**client
                    }));
                }

                self.arrange_clients();
            }
        }
    }

    fn handle_move_window(&mut self, event: &XEvent) {
        let clean_mask = self.xlib_state.clean_mask();

        if unsafe {
            self.move_window.is_none()
                && event.get_type() == xlib::ButtonPress
                && event.button.button == 1
                && event.button.state & clean_mask == Mod1Mask & clean_mask
                && event.button.subwindow != 0
        } {
            let win_pos = unsafe {
                let mut attr: xlib::XWindowAttributes =
                    std::mem::MaybeUninit::zeroed().assume_init();
                xlib::XGetWindowAttributes(self.dpy(), event.button.subwindow, &mut attr);

                (attr.x, attr.y)
            };

            self.move_window = Some(unsafe {
                (
                    event.button.subwindow,
                    (event.button.x, event.button.y),
                    win_pos,
                )
            });

            if let Some(client) = self.client_for_window(event.button.subwindow) {
                self.clients.replace(Client::new_rc(InnerClient {
                    floating: true,
                    ..**client
                }));
            }

            self.arrange_clients();
        } else if unsafe {
            self.move_window.is_some()
                && event.get_type() == xlib::ButtonRelease
                && event.button.button == 1
        } {
            self.move_window = None;
        } else if self.move_window.is_some() && event.get_type() == xlib::MotionNotify {
            let move_window = self.move_window.unwrap();

            let attr = unsafe {
                let mut attr: xlib::XWindowAttributes =
                    std::mem::MaybeUninit::zeroed().assume_init();
                xlib::XGetWindowAttributes(self.dpy(), move_window.0, &mut attr);

                attr
            };

            let (x, y) = unsafe {
                (
                    event.motion.x - move_window.1 .0 + move_window.2 .0,
                    event.motion.y - move_window.1 .1 + move_window.2 .1,
                )
            };

            let mut wc = xlib::XWindowChanges {
                x,
                y,
                width: attr.width,
                height: attr.height,
                border_width: 0,
                sibling: 0,
                stack_mode: 0,
            };

            unsafe {
                xlib::XConfigureWindow(
                    self.dpy(),
                    move_window.0,
                    (xlib::CWX | xlib::CWY) as u32,
                    &mut wc,
                );

                xlib::XSync(self.dpy(), 0);
            }
        }
    }

    fn handle_resize_window(&mut self, event: &XEvent) {
        let clean_mask = self.xlib_state.clean_mask();

        let resize_window = self.resize_window;

        if unsafe {
            resize_window.is_none()
                && event.get_type() == xlib::ButtonPress
                && event.button.button == 3
                && event.button.state & clean_mask == Mod1Mask & clean_mask
                && event.button.subwindow != 0
        } {
            unsafe {
                let mut attr: xlib::XWindowAttributes =
                    std::mem::MaybeUninit::zeroed().assume_init();

                xlib::XGetWindowAttributes(self.dpy(), event.button.subwindow, &mut attr);

                self.resize_window = Some((event.button.subwindow, (attr.x, attr.y)));

                xlib::XWarpPointer(
                    self.dpy(),
                    0,
                    event.button.subwindow,
                    0,
                    0,
                    0,
                    0,
                    attr.width + attr.border_width - 1,
                    attr.height + attr.border_width - 1,
                );
            };

            if let Some(client) = self.client_for_window(unsafe { event.button.subwindow }) {
                self.clients.replace(Client::new_rc(InnerClient {
                    floating: true,
                    ..**client
                }));
            }

            self.arrange_clients();
        } else if unsafe {
            resize_window.is_some()
                && event.get_type() == xlib::ButtonRelease
                && event.button.button == 3
        } {
            self.resize_window = None;
        } else if resize_window.is_some() && event.get_type() == xlib::MotionNotify {
            let resize_window = resize_window.unwrap();

            let attr = unsafe {
                let mut attr: xlib::XWindowAttributes =
                    std::mem::MaybeUninit::zeroed().assume_init();
                xlib::XGetWindowAttributes(self.dpy(), resize_window.0, &mut attr);

                attr
            };

            unsafe {
                let (width, height) = {
                    (
                        std::cmp::max(
                            1,
                            event.motion.x - resize_window.1 .0 + 2 * attr.border_width + 1,
                        ),
                        std::cmp::max(
                            1,
                            event.motion.y - resize_window.1 .1 + 2 * attr.border_width + 1,
                        ),
                    )
                };

                let mut wc = xlib::XWindowChanges {
                    x: attr.x,
                    y: attr.y,
                    width,
                    height,
                    border_width: attr.border_width,
                    sibling: 0,
                    stack_mode: 0,
                };

                xlib::XConfigureWindow(
                    self.dpy(),
                    resize_window.0,
                    (xlib::CWWidth | xlib::CWHeight) as u32,
                    &mut wc,
                );

                xlib::XSync(self.dpy(), 0);
            }
        }
    }

    // spawn a new process / calls execvp
    pub fn spawn<T: ToString>(&self, command: T, args: &[T]) -> Result<()> {
        let fd = unsafe { xlib::XConnectionNumber(self.dpy()) };

        match unsafe { fork() } {
            Ok(ForkResult::Parent { .. }) => Ok(()),
            Ok(ForkResult::Child) => {
                // i dont think i want to exit this block without closing the program,
                // so unwrap everything

                close(fd)
                    .or_else(|_| Err("failed to close x connection"))
                    .unwrap();
                setsid().ok().ok_or("failed to setsid").unwrap();

                let c_cmd = CString::new(command.to_string()).unwrap();

                let c_args: Vec<_> = args
                    .iter()
                    .map(|s| CString::new(s.to_string()).unwrap())
                    .collect();

                execvp(
                    &c_cmd,
                    &c_args.iter().map(|s| s.as_c_str()).collect::<Vec<_>>(),
                )
                .or(Err("failed to execvp()"))
                .unwrap();

                eprintln!("execvp({}) failed.", c_cmd.to_str().unwrap());
                std::process::exit(0);
            }
            Err(_) => Err(Error::new(ErrorKind::Other, "failed to fork.")),
        }
    }
}
*/
