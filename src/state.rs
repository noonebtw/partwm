use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::CString,
    ptr::{null, null_mut},
    rc::{Rc, Weak},
};

use x11::xlib::{
    self, Atom, ControlMask, LockMask, Mod1Mask, Mod2Mask, Mod3Mask, Mod4Mask, Mod5Mask, ShiftMask,
    Window, XDefaultScreen, XEvent, XInternAtom, XOpenDisplay, XRootWindow,
};
use xlib::GrabModeAsync;

use log::info;

use crate::util::BuildIdentityHasher;

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

#[derive(Clone, Debug)]
pub struct Client {
    window: Window,
    floating: bool,
    size: (i32, i32),
    position: (i32, i32),
}

impl Default for Client {
    fn default() -> Self {
        Self {
            window: 0,
            floating: false,
            size: (0, 0),
            position: (0, 0),
        }
    }
}

impl Client {
    pub fn new(window: xlib::Window) -> Self {
        Self {
            window,
            ..Default::default()
        }
    }
}

impl PartialEq for Client {
    fn eq(&self, other: &Self) -> bool {
        self.window == other.window
    }
}

impl Eq for Client {}

#[derive(Clone, Debug)]
struct VirtualScreen {
    master_stack: HashMap<Window, Weak<RefCell<Client>>, BuildIdentityHasher>,
    aux_stack: HashMap<Window, Weak<RefCell<Client>>, BuildIdentityHasher>,
    focused_client: Weak<RefCell<Client>>,
}

impl VirtualScreen {
    fn new() -> Self {
        Self {
            master_stack: HashMap::default(),
            aux_stack: HashMap::default(),
            focused_client: Weak::new(),
        }
    }

    fn contains_client(&self, client: Rc<RefCell<Client>>) -> bool {
        self.master_stack.contains_key(&client.borrow().window)
            || self.aux_stack.contains_key(&client.borrow().window)
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

struct XLibState {
    display: Display,
    root: Window,
    screen: i32,
    // atoms
    atoms: XLibAtoms,
}

impl XLibState {
    fn new() -> Self {
        let (display, screen, root) = unsafe {
            let display = XOpenDisplay(null());
            assert_ne!(display, null_mut());

            let screen = XDefaultScreen(display);
            let root = XRootWindow(display, screen);

            (Display::new(display), screen, root)
        };

        Self {
            atoms: XLibAtoms::init(display.clone()),
            display,
            root,
            screen,
        }
    }

    fn dpy(&self) -> *mut x11::xlib::Display {
        self.display.get()
    }

    fn root(&self) -> Window {
        self.root
    }

    fn screen(&self) -> i32 {
        self.screen
    }

    pub fn grab_key(&self, window: xlib::Window, keycode: i32, mod_mask: u32) {
        let numlock_mask = self.get_numlock_mask();
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
        let numlock_mask = self.get_numlock_mask();
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

    pub fn keycode(&self, string: &str) -> i32 {
        let c_string = CString::new(string).unwrap();
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

    fn send_event(&self, window: xlib::Window, proto: Option<xlib::Atom>) -> bool {
        if proto.is_some() && self.check_for_protocol(window, proto.unwrap()) {
            let mut data = xlib::ClientMessageData::default();
            data.set_long(0, proto.unwrap() as i64);
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

            return true;
        }

        return false;
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
                        == xlib::XKeysymToKeycode(self.dpy(), x11::keysym::XK_Num_Lock as u64)
                    {
                        return 1 << i;
                    }
                }
            }
        }

        0
    }

    fn clean_mod_mask(&self) -> u32 {
        !(self.get_numlock_mask() | LockMask)
            & (ShiftMask | ControlMask | Mod1Mask | Mod2Mask | Mod3Mask | Mod4Mask | Mod5Mask)
    }
}

trait DisplayServer {
    type Window;

    fn grab_key(&self, window: Self::Window, keycode: i32, mod_mask: u32);
    fn grab_button(&self, window: Self::Window, keycode: i32, button_mask: u32, mod_mask: u32);
}

pub struct WMState {
    xlib_state: XLibState,
    key_handlers: Vec<(i32, u32, Rc<dyn Fn(&Self, &XEvent)>)>,
    // (button, mod_mask, button_mask)
    buttons: Vec<(u32, u32, i64)>,
    event_handlers: Vec<Rc<dyn Fn(&Self, &XEvent)>>,

    //move_window:
    // u64 : window to move
    // (i32, i32) : initial cursor position
    // (i32, i32) : initial window position
    move_window: Option<(u64, (i32, i32), (i32, i32))>,
    //resize_window:
    // u64 : window to move
    // (i32, i32) : initial window position
    resize_window: Option<(u64, (i32, i32))>,
    clients: HashMap<Window, Rc<RefCell<Client>>>,
    focused_client: Weak<RefCell<Client>>,
    current_vscreen: usize,
    virtual_screens: Vec<VirtualScreen>,
}

impl WMState {
    fn stack_unstacked_clients(&mut self) {
        info!("[stack_unstacked_clients] ");
        let current_vscreen = self.current_vscreen;

        self.clients
            .iter()
            .filter(|(w, c)| !c.borrow().floating && !self.is_client_stacked(w))
            .map(|(w, c)| (w.clone(), Rc::downgrade(c)))
            .collect::<Vec<(u64, Weak<RefCell<Client>>)>>()
            .iter()
            .for_each(|(w, c)| {
                info!(
                    "[stack_unstacked_clients] inserting Window({:?}) into aux_stack",
                    w
                );

                self.virtual_screens[current_vscreen]
                    .aux_stack
                    .insert(w.clone(), c.clone());
            });
    }

    fn is_client_stacked(&self, window: &Window) -> bool {
        self.virtual_screens
            .iter()
            .any(|vs| vs.contains_window(window))
    }

    fn client_for_window(&self, window: &Window) -> Option<Rc<RefCell<Client>>> {
        self.clients
            .iter()
            .filter(|&(w, _)| *w == *window)
            .next()
            .map(|(_, c)| c.clone())
    }

    fn switch_stack_for_client(&mut self, window: &Window) {
        if let Some(client) = self.client_for_window(window) {
            info!("[switch_stack_for_client] client: {:#?}", client.borrow());
            client.borrow_mut().floating = false;

            if self.virtual_screens[self.current_vscreen]
                .master_stack
                .contains_key(window)
            {
                self.virtual_screens[self.current_vscreen]
                    .master_stack
                    .remove(window);
                self.virtual_screens[self.current_vscreen]
                    .aux_stack
                    .insert(*window, Rc::downgrade(&client));
                info!("[switch_stack_for_client] moved to aux stack");
            } else {
                self.virtual_screens[self.current_vscreen]
                    .aux_stack
                    .remove(window);
                self.virtual_screens[self.current_vscreen]
                    .master_stack
                    .insert(*window, Rc::downgrade(&client));
                info!("[switch_stack_for_client] moved to master stack");
            }
        }
    }

    fn refresh_screen(&mut self) {
        let current_vscreen = self.current_vscreen;

        self.virtual_screens
            .get_mut(current_vscreen)
            .and_then(|vs| {
                vs.master_stack.retain(|_, c| {
                    c.upgrade().is_some() && !c.upgrade().unwrap().borrow().floating
                });
                vs.aux_stack.retain(|_, c| {
                    c.upgrade().is_some() && !c.upgrade().unwrap().borrow().floating
                });

                Some(())
            });

        self.stack_unstacked_clients();

        if self.virtual_screens[current_vscreen]
            .master_stack
            .is_empty()
        {
            info!("[refresh_screen] master stack was empty, pushing first client if exists:");

            self.virtual_screens[current_vscreen]
                .aux_stack
                .iter()
                .filter(|(_, c)| !c.upgrade().unwrap().borrow().floating)
                .next()
                .map(|(w, c)| (w.clone(), c.clone()))
                .and_then(|(w, c)| {
                    info!("[arrange_clients] Window({:#?})", w);

                    self.virtual_screens[current_vscreen]
                        .master_stack
                        .insert(w, c);
                    self.virtual_screens[current_vscreen].aux_stack.remove(&w)
                });
        }
    }
}
