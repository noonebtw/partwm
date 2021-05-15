#![allow(non_upper_case_globals)]
pub mod keysym;

use std::ptr::null;

use x11::xlib::{
    ButtonPress, ButtonRelease, ConfigureRequest, CreateNotify,
    DestroyNotify, EnterNotify, KeyPress, KeyRelease, MapRequest,
    MotionNotify, UnmapNotify, Window, XAnyEvent, XButtonEvent,
    XConfigureRequestEvent, XCreateWindowEvent, XDestroyWindowEvent,
    XEvent, XKeyEvent, XLookupKeysym, XMapRequestEvent, XMotionEvent,
    XNextEvent, XRootWindow, XUnmapEvent,
};

use crate::backends::window_event::{
    ButtonEvent, KeyEvent, KeyState, ModifierKey,
};

use self::keysym::{keysym_to_virtual_keycode, xev_to_mouse_button};

use super::window_event::{
    ConfigureEvent, CreateEvent, DestroyEvent, MapEvent,
    ModifierState, MotionEvent, UnmapEvent, WindowEvent,
};

// xlib backend
pub struct XLib {
    display: *mut x11::xlib::Display,
    modifier_state: ModifierState,
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
            modifier_state: Default::default(),
        }
    }

    fn root_window(&self) -> Window {
        unsafe { XRootWindow(self.display, self.screen) }
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
                    self.modifier_state.set_modifier(modifier)
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
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
