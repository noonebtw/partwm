// asdf
use std::{
    cell::{Cell, RefCell, RefMut},
    collections::HashMap,
    ffi::CString,
    hash::{Hash, Hasher},
    io::{Error, ErrorKind, Result},
    ptr::{null, null_mut},
    sync::Arc,
};

use x11::{
    xlib,
    xlib::{
        Atom, ButtonPressMask, ButtonReleaseMask, CWEventMask, ControlMask, EnterWindowMask,
        GrabModeAsync, LeaveWindowMask, LockMask, Mod1Mask, Mod2Mask, Mod3Mask, Mod4Mask, Mod5Mask,
        PointerMotionMask, PropertyChangeMask, ShiftMask, StructureNotifyMask,
        SubstructureNotifyMask, SubstructureRedirectMask, Window, XDefaultScreen, XErrorEvent,
        XEvent, XInternAtom, XOpenDisplay, XRootWindow,
    },
};

use nix::unistd::{close, execvp, fork, setsid, ForkResult};

#[derive(Clone)]
pub struct Display(Arc<Cell<*mut x11::xlib::Display>>);

impl Display {
    pub fn new(display: *mut x11::xlib::Display) -> Self {
        Self {
            0: Arc::new(Cell::new(display)),
        }
    }

    pub fn get(&self) -> *mut x11::xlib::Display {
        self.0.get()
    }
}

pub struct WMAtoms {
    pub protocols: Option<Atom>,
    pub delete: Option<Atom>,
}

impl WMAtoms {
    pub fn init(display: Display) -> Self {
        Self {
            protocols: {
                Some(unsafe {
                    let wm_protocols_str = CString::new("WM_PROTOCOLS").unwrap();
                    XInternAtom(display.get(), wm_protocols_str.as_c_str().as_ptr(), 0)
                })
                .filter(|&atom| atom != 0)
            },
            delete: {
                Some(unsafe {
                    let wm_delete_str = CString::new("WM_DELETE_WINDOW").unwrap();
                    XInternAtom(display.get(), wm_delete_str.as_c_str().as_ptr(), 0)
                })
                .filter(|&atom| atom != 0)
            },
        }
    }
}

impl Default for WMAtoms {
    fn default() -> Self {
        Self {
            protocols: None,
            delete: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Client {
    window: Window,
}

impl PartialEq for Client {
    fn eq(&self, other: &Self) -> bool {
        self.window == other.window
    }
}

impl Eq for Client {}

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
            display: display.clone(),
            root,
            screen,
            atoms: WMAtoms::init(display),
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

    pub fn grab_key(&self, window: xlib::Window, keycode: i32, mask: u32) {
        let numlock_mask = self.numlock_mask();
        let modifiers = vec![0, LockMask, numlock_mask, LockMask | numlock_mask];
        for &modifier in modifiers.iter() {
            unsafe {
                xlib::XGrabKey(
                    self.dpy(),
                    keycode,
                    mask | modifier,
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
        if self.check_for_protocol(window, proto) && self.atoms.protocols.is_some() {
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
                    message_type: self.atoms.protocols.unwrap(),
                    data,
                },
            };

            unsafe {
                xlib::XSendEvent(self.dpy(), window, 0, xlib::NoEventMask, &mut event);
            }

            return true;
        }

        return false;
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

#[derive(Clone)]
pub struct WMStateMut {
    //move_window:
    // u64 : window to move
    // (i32, i32) : initial cursor position
    // (i32, i32) : initial window position
    move_window: Option<(u64, (i32, i32), (i32, i32))>,
    //resize_window:
    // u64 : window to move
    // (i32, i32) : initial window position
    resize_window: Option<(u64, (i32, i32))>,
    clients: HashMap<Window, Arc<Client>>,
}

impl Default for WMStateMut {
    fn default() -> Self {
        Self {
            move_window: None,
            resize_window: None,
            clients: HashMap::new(),
        }
    }
}

pub struct WMState {
    xlib_state: XLibState,
    key_handlers: Vec<(i32, u32, Arc<dyn Fn(&Self, &XEvent)>)>,
    event_handlers: Vec<Arc<dyn Fn(&Self, &XEvent)>>,
    mut_state: RefCell<WMStateMut>,
}

impl WMState {
    pub fn new() -> Self {
        Self {
            xlib_state: XLibState::new(),
            mut_state: RefCell::new(WMStateMut::default()),
            key_handlers: vec![],
            event_handlers: vec![],
        }
    }

    pub fn init() -> Self {
        let state = Self::new()
            .grab_button(
                1,
                Mod1Mask,
                ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
            )
            .add_event_handler(Self::handle_move_window)
            .grab_button(
                3,
                Mod1Mask,
                ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
            )
            .add_event_handler(Self::handle_resize_window)
            .add_key_handler("T", Mod1Mask, |state, _| {
                println!("spawning terminal");
                let _ = state.spawn("xterm", &[]);
            })
            .add_key_handler("E", Mod1Mask, |state, _| {
                println!("spawning emacs");
                let _ = state.spawn("emacs", &[]);
            })
            .add_key_handler("L", Mod1Mask, |state, _| {
                let _ = state.mut_state.try_borrow().and_then(|mut_state| {
                    mut_state.clients.iter().for_each(|(_, client)| {
                        println!("{:?}", client);
                    });

                    Ok(())
                });
            })
            .add_key_handler("Q", Mod1Mask, |state, event| unsafe {
                if event.key.subwindow != 0 {
                    if state.xlib_state.atoms.delete.is_none()
                        || !state
                            .xlib_state
                            .send_event(event.key.subwindow, state.xlib_state.atoms.delete.unwrap())
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
            wa.event_mask = SubstructureRedirectMask | StructureNotifyMask | SubstructureNotifyMask;

            xlib::XChangeWindowAttributes(state.dpy(), state.root(), CWEventMask, &mut wa);
        }

        state
    }

    pub fn run(self) -> Self {
        loop {
            let event = unsafe {
                let mut event: xlib::XEvent = std::mem::MaybeUninit::zeroed().assume_init();
                xlib::XNextEvent(self.dpy(), &mut event);

                event
            };

            match event.get_type() {
                xlib::MapRequest => {
                    let event = unsafe { &event.map_request };

                    let _ = self.mut_state.try_borrow_mut().and_then(|mut_state| {
                        RefMut::map(mut_state, |t| &mut t.clients)
                            .entry(event.window)
                            .or_insert_with(|| {
                                unsafe { xlib::XMapWindow(self.dpy(), event.window) };
                                Arc::new(Client {
                                    window: event.window,
                                })
                            });

                        Ok(())
                    });
                }
                xlib::UnmapNotify => {
                    let event = unsafe { &event.unmap };

                    println!("UnmapNotify: {:?}", event.window);

                    if event.send_event == 0 {
                        let _ = self.mut_state.try_borrow_mut().and_then(|mut_state| {
                            if mut_state.clients.contains_key(&event.window) {
                                RefMut::map(mut_state, |t| &mut t.clients).remove(&event.window);
                            }

                            Ok(())
                        });
                    }
                }
                xlib::DestroyNotify => {
                    let event = unsafe { &event.destroy_window };

                    println!("DestroyNotify: {:?}", event.window);

                    let _ = self.mut_state.try_borrow_mut().and_then(|mut_state| {
                        if mut_state.clients.contains_key(&event.window) {
                            RefMut::map(mut_state, |t| &mut t.clients).remove(&event.window);
                        }

                        Ok(())
                    });
                }
                xlib::ConfigureRequest => {
                    let event = unsafe { &event.configure_request };

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
                        xlib::XSync(self.dpy(), 0);
                    }
                }
                xlib::KeyPress => {
                    let clean_mask = self.xlib_state.clean_mask();

                    self.key_handlers.iter().for_each(|(key, mask, handler)| {
                        if unsafe {
                            event.key.keycode == *key as u32
                                && event.key.state & clean_mask == *mask & clean_mask
                        } {
                            handler(&self, &event);
                        }
                    })
                }
                _ => self.event_handlers.iter().for_each(|handler| {
                    handler(&self, &event);
                }),
            }
        }
    }

    pub fn dpy(&self) -> *mut xlib::Display {
        self.xlib_state.dpy()
    }

    pub fn root(&self) -> xlib::Window {
        self.xlib_state.root()
    }

    pub fn grab_button(self, button: u32, mod_mask: u32, button_mask: i64) -> Self {
        self.xlib_state
            .grab_button(self.root(), button, mod_mask, button_mask);

        self
    }

    pub fn add_event_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&Self, &XEvent) + 'static,
    {
        self.event_handlers.push(Arc::new(handler));

        self
    }

    pub fn add_key_handler<S, F>(mut self, key: S, mask: u32, handler: F) -> Self
    where
        S: Into<String>,
        F: Fn(&Self, &XEvent) + 'static,
    {
        let keycode = self.xlib_state.keycode(key);

        self.key_handlers.push((keycode, mask, Arc::new(handler)));
        self.xlib_state.grab_key(self.root(), keycode, mask);

        self
    }

    fn handle_move_window(&self, event: &XEvent) {
        let clean_mask = self.xlib_state.clean_mask();

        let move_window = &mut self.mut_state.borrow_mut().move_window;

        if unsafe {
            move_window.is_none()
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

            *move_window = Some(unsafe {
                (
                    event.button.subwindow,
                    (event.button.x, event.button.y),
                    win_pos,
                )
            });
        } else if unsafe {
            move_window.is_some()
                && event.get_type() == xlib::ButtonRelease
                && event.button.button == 1
        } {
            *move_window = None;
        } else if move_window.is_some() && event.get_type() == xlib::MotionNotify {
            let move_window = move_window.unwrap();

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

    fn handle_resize_window(&self, event: &XEvent) {
        let clean_mask = self.xlib_state.clean_mask();

        let resize_window = &mut self.mut_state.borrow_mut().resize_window;

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

                *resize_window = Some((event.button.subwindow, (attr.x, attr.y)));

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
        } else if unsafe {
            resize_window.is_some()
                && event.get_type() == xlib::ButtonRelease
                && event.button.button == 3
        } {
            *resize_window = None;
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
