#![allow(non_upper_case_globals)]
pub mod keysym;

use std::ptr::null;

use x11::xlib::{
    AnyModifier, Atom, ButtonPress, ButtonPressMask, ButtonRelease,
    ButtonReleaseMask, ClientMessage, ConfigureRequest, CreateNotify,
    DestroyNotify, EnterWindowMask, FocusChangeMask, GrabModeAsync,
    KeyPress, KeyPressMask, KeyRelease, KeyReleaseMask, MapNotify,
    MapRequest, MotionNotify, PropertyChangeMask, PropertyNewValue,
    PropertyNotify, StructureNotifyMask, UnmapNotify, Window,
    XButtonEvent, XClientMessageEvent, XConfigureRequestEvent,
    XConfigureWindow, XCreateWindowEvent, XDestroyWindowEvent,
    XEvent, XGrabButton, XGrabKey, XInternAtom, XKeyEvent,
    XKeysymToKeycode, XLookupKeysym, XMapRequestEvent, XMapWindow,
    XMotionEvent, XNextEvent, XPropertyEvent, XRootWindow,
    XSelectInput, XUnmapEvent, XWindowChanges,
};

use crate::backends::window_event::{
    ButtonEvent, KeyEvent, KeyState, ModifierKey,
};

use self::keysym::{
    keysym_to_virtual_keycode, mouse_button_to_xbutton,
    virtual_keycode_to_keysym, xev_to_mouse_button,
};

use super::{
    keycodes::{MouseButton, VirtualKeyCode},
    window_event::{
        ConfigureEvent, CreateEvent, DestroyEvent, FullscreenEvent,
        MapEvent, ModifierState, MotionEvent, UnmapEvent,
        WindowEvent,
    },
};

struct Atoms {
    wm_protocols: Atom,
    wm_state: Atom,
    wm_delete_window: Atom,
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

// xlib backend
pub struct XLib {
    display: *mut x11::xlib::Display,
    modifier_state: ModifierState,
    atoms: Atoms,
    screen: i32,
}

impl Drop for XLib {
    fn drop(&mut self) {
        unsafe {
            x11::xlib::XCloseDisplay(self.display);
        }
    }
}

impl XLib {
    pub fn new() -> Self {
        let (display, screen) = unsafe {
            let display = x11::xlib::XOpenDisplay(null());
            let screen = x11::xlib::XDefaultScreen(display);

            (display, screen)
        };

        Self {
            display,
            screen,
            atoms: Self::init_atoms(display),
            modifier_state: Default::default(),
        }
    }

    fn root_window(&self) -> Window {
        unsafe { XRootWindow(self.display, self.screen) }
    }

    fn init_atoms(display: *mut x11::xlib::Display) -> Atoms {
        unsafe {
            let wm_protocols = XInternAtom(
                display,
                b"WM_PROTOCOLS\0".as_ptr() as *const _,
                0,
            );
            let wm_state = XInternAtom(
                display,
                b"WM_STATE\0".as_ptr() as *const _,
                0,
            );
            let wm_delete_window = XInternAtom(
                display,
                b"WM_DELETE_WINDOW\0".as_ptr() as *const _,
                0,
            );
            let wm_take_focus = XInternAtom(
                display,
                b"WM_TAKE_FOCUS\0".as_ptr() as *const _,
                0,
            );
            let net_supported = XInternAtom(
                display,
                b"_NET_SUPPORTED\0".as_ptr() as *const _,
                0,
            );
            let net_active_window = XInternAtom(
                display,
                b"_NET_ACTIVE_WINDOW\0".as_ptr() as *const _,
                0,
            );
            let net_client_list = XInternAtom(
                display,
                b"_NET_CLIENT_LIST\0".as_ptr() as *const _,
                0,
            );
            let net_wm_name = XInternAtom(
                display,
                b"_NET_WM_NAME\0".as_ptr() as *const _,
                0,
            );
            let net_wm_state = XInternAtom(
                display,
                b"_NET_WM_STATE\0".as_ptr() as *const _,
                0,
            );
            let net_wm_state_fullscreen = XInternAtom(
                display,
                b"_NET_WM_STATE_FULLSCREEN\0".as_ptr() as *const _,
                0,
            );
            let net_wm_window_type = XInternAtom(
                display,
                b"_NET_WM_WINDOW_TYPE\0".as_ptr() as *const _,
                0,
            );
            let net_wm_window_type_dialog = XInternAtom(
                display,
                b"_NET_WM_WINDOW_TYPE_DIALOG\0".as_ptr() as *const _,
                0,
            );

            Atoms {
                wm_protocols,
                wm_state,
                wm_delete_window,
                wm_take_focus,
                net_supported,
                net_active_window,
                net_client_list,
                net_wm_name,
                net_wm_state,
                net_wm_state_fullscreen,
                net_wm_window_type,
                net_wm_window_type_dialog,
            }
        }
    }

    fn update_modifier_state(
        &mut self,
        keyevent: &x11::xlib::XKeyEvent,
    ) {
        //keyevent.keycode
        let keysym = self.keyev_to_keysym(keyevent);

        use x11::keysym::*;

        let modifier = match keysym as u32 {
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
                KeyPress => {
                    self.modifier_state.set_modifier(modifier)
                }
                KeyRelease => {
                    self.modifier_state.unset_modifier(modifier)
                }
                _ => unreachable!(
                    "keyyevent != (KeyPress | KeyRelease)"
                ),
            }
        }
    }

    fn keyev_to_keysym(&self, keyev: &XKeyEvent) -> u32 {
        unsafe {
            XLookupKeysym(keyev as *const _ as *mut _, 0) as u32
        }
    }

    pub fn next_event(&self) -> XEvent {
        unsafe {
            let mut event = std::mem::MaybeUninit::zeroed();
            XNextEvent(self.display, event.as_mut_ptr());

            event.assume_init()
        }
    }

    /// should probabbly make this use some variable that the user can chose for selected events.
    fn map_window(&self, window: Window) {
        unsafe {
            XMapWindow(self.display, window);

            XSelectInput(
                self.display,
                window,
                EnterWindowMask
                    | FocusChangeMask
                    | PropertyChangeMask
                    | StructureNotifyMask,
            );
        }
    }

    fn select_input(&self, window: Window) {
        unsafe {
            XSelectInput(
                self.display,
                window,
                EnterWindowMask
                    | FocusChangeMask
                    | PropertyChangeMask
                    | StructureNotifyMask
                    | ButtonPressMask
                    | ButtonReleaseMask
                    | KeyPressMask
                    | KeyReleaseMask,
            );
        }
    }

    fn configure_window(
        &self,
        window: Window,
        event: &ConfigureEvent,
    ) {
        unsafe {
            let mut wc =
                std::mem::MaybeUninit::<XWindowChanges>::zeroed()
                    .assume_init();

            wc.x = event.position[0];
            wc.y = event.position[1];

            wc.width = event.size[0];
            wc.height = event.size[1];

            XConfigureWindow(
                self.display,
                window,
                (1 << 4) - 1,
                &mut wc,
            );
        }
    }

    fn handle_window_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::MapEvent { window, .. } => {
                self.map_window(window);
            }
            WindowEvent::ConfigureEvent { window, event } => {
                self.configure_window(window, &event);
            }
            _ => {}
        }
    }

    fn grab_key(&self, keycode: VirtualKeyCode) {
        unsafe {
            XGrabKey(
                self.display,
                XKeysymToKeycode(
                    self.display,
                    virtual_keycode_to_keysym(keycode).unwrap()
                        as u64,
                ) as i32,
                AnyModifier,
                self.root_window(),
                1,
                GrabModeAsync,
                GrabModeAsync,
            );
        }
    }

    fn grab_button(&self, button: MouseButton) {
        unsafe {
            XGrabButton(
                self.display,
                mouse_button_to_xbutton(button) as u32,
                AnyModifier,
                self.root_window(),
                1,
                (ButtonPressMask | ButtonReleaseMask) as u32,
                GrabModeAsync,
                GrabModeAsync,
                0,
                0,
            );
        }
    }

    fn next_window_event(&mut self) -> WindowEvent {
        loop {
            let event = self.next_event();

            match event.get_type() {
                KeyPress | KeyRelease => {
                    let key_ev: &XKeyEvent = event.as_ref();

                    self.update_modifier_state(key_ev);

                    let keycode = keysym_to_virtual_keycode(
                        self.keyev_to_keysym(event.as_ref()),
                    );

                    if let Some(keycode) = keycode {
                        return WindowEvent::KeyEvent {
                            window: key_ev.subwindow,
                            event: KeyEvent::new(
                                match event.get_type() {
                                    KeyPress => KeyState::Pressed,
                                    KeyRelease => KeyState::Released,
                                    _ => unreachable!(),
                                },
                                keycode,
                                self.modifier_state.clone(),
                            ),
                        };
                    }
                }
                ButtonPress | ButtonRelease => {
                    let button_ev: &XButtonEvent = event.as_ref();
                    let button = xev_to_mouse_button(button_ev);

                    if let Some(button) = button {
                        return WindowEvent::ButtonEvent {
                            window: button_ev.subwindow,
                            event: ButtonEvent::new(
                                match event.get_type() {
                                    ButtonPress => KeyState::Pressed,
                                    ButtonRelease => {
                                        KeyState::Released
                                    }
                                    _ => unreachable!(),
                                },
                                button,
                                self.modifier_state.clone(),
                            ),
                        };
                    }
                }
                MotionNotify => {
                    let motion_ev: &XMotionEvent = event.as_ref();

                    return WindowEvent::MotionEvent {
                        window: motion_ev.subwindow,
                        event: MotionEvent::new([
                            motion_ev.x_root,
                            motion_ev.y_root,
                        ]),
                    };
                }
                MapRequest => {
                    // MapEvent
                    let map_ev: &XMapRequestEvent = event.as_ref();

                    return WindowEvent::MapEvent {
                        window: map_ev.window,
                        event: MapEvent::new(map_ev.window),
                    };
                }
                MapNotify => {
                    // MapEvent
                    let map_ev: &XMapRequestEvent = event.as_ref();

                    return WindowEvent::MapEvent {
                        window: map_ev.window,
                        event: MapEvent::new(map_ev.window),
                    };
                }
                UnmapNotify => {
                    // UnmapEvent
                    let unmap_ev: &XUnmapEvent = event.as_ref();

                    return WindowEvent::UnmapEvent {
                        window: unmap_ev.window,
                        event: UnmapEvent::new(unmap_ev.window),
                    };
                }
                CreateNotify => {
                    // CreateEvent
                    let create_ev: &XCreateWindowEvent =
                        event.as_ref();

                    return WindowEvent::CreateEvent {
                        window: create_ev.window,
                        event: CreateEvent::new(
                            create_ev.window,
                            [create_ev.x, create_ev.y],
                            [create_ev.width, create_ev.height],
                        ),
                    };
                }
                DestroyNotify => {
                    // DestroyEvent
                    let destroy_ev: &XDestroyWindowEvent =
                        event.as_ref();

                    return WindowEvent::DestroyEvent {
                        window: destroy_ev.window,
                        event: DestroyEvent::new(destroy_ev.window),
                    };
                }
                ConfigureRequest => {
                    // ConfigureEvent
                    let configure_ev: &XConfigureRequestEvent =
                        event.as_ref();

                    return WindowEvent::ConfigureEvent {
                        window: configure_ev.window,
                        event: ConfigureEvent::new(
                            configure_ev.window,
                            [configure_ev.x, configure_ev.y],
                            [configure_ev.width, configure_ev.height],
                        ),
                    };
                }
                ClientMessage => {
                    let msg_ev: &XClientMessageEvent = event.as_ref();

                    // not sure?
                }
                PropertyNotify => {
                    let property_ev: &XPropertyEvent = event.as_ref();

                    if property_ev.atom
                        == self.atoms.net_wm_state_fullscreen
                    {
                        return WindowEvent::FullscreenEvent {
                            window: property_ev.window,
                            event: FullscreenEvent::new(
                                property_ev.state == PropertyNewValue,
                            ),
                        };
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use x11::xlib::{
        XBlackPixel, XCreateSimpleWindow, XCreateWindow,
        XDefaultScreen,
    };

    use super::*;

    #[test]
    fn window_events() {
        let mut xlib = XLib::new();

        //xlib.grab_key(VirtualKeyCode::A);

        let window = unsafe {
            //XCreateWindow(xlib.display, , 10, 9, 8, 7, 6, 5, 4, 3, 2, 1)
            let black_pixel = XBlackPixel(
                xlib.display,
                XDefaultScreen(xlib.display),
            );
            let window = XCreateSimpleWindow(
                xlib.display,
                xlib.root_window(),
                10,
                10,
                100,
                100,
                1,
                black_pixel,
                black_pixel,
            );

            XMapWindow(xlib.display, window);
            xlib.select_input(window);

            window
        };

        loop {
            let event = xlib.next_window_event();
            println!("{:#?}", event);
        }
    }

    //#[test]
    // fn window_events() {
    //     let mut xlib = XLib::new();

    //     loop {
    //         if let Some(event) =
    //             xlib.xevent_to_window_event(xlib.next_event())
    //         {
    //             println!("{:#?}", event);
    //         }
    //     }
    // }
}
