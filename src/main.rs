use x11::xlib;

use std::ffi::CString;
use std::io::{Error, ErrorKind, Result};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Arc;

use x11::xlib::{ButtonPressMask, ButtonReleaseMask, GrabModeAsync, PointerMotionMask, XEvent};

use x11::xlib::{
    ControlMask, LockMask, Mod1Mask, Mod2Mask, Mod3Mask, Mod4Mask, Mod5Mask, ShiftMask,
};

use nix::unistd::{close, execvp, fork, setsid, ForkResult};

type Display = Arc<AtomicPtr<xlib::Display>>;

struct XlibState {
    display: Display,
    keys: Vec<(i32, u32, Box<dyn Fn(&Self, &XEvent)>)>,
    //move_window:
    // u64 : window to move
    // (i32, i32) : initial cursor position
    // (i32, i32) : initial window position
    move_window: Option<(u64, (i32, i32), (i32, i32))>,
    //resize_window:
    // u64 : window to move
    // (i32, i32) : initial window position
    resize_window: Option<(u64, (i32, i32))>,
}

impl XlibState {
    fn new() -> Result<Self> {
        let display = unsafe { xlib::XOpenDisplay(null()) };
        assert_ne!(display, null_mut());

        let display = Display::new(AtomicPtr::new(display));

        Ok(Self {
            display,
            keys: vec![],
            move_window: None,
            resize_window: None,
        })
    }

    fn dpy(&self) -> *mut xlib::Display {
        self.display.load(Ordering::SeqCst)
    }

    fn root(&self) -> u64 {
        unsafe { xlib::XDefaultRootWindow(self.dpy()) }
    }

    #[allow(dead_code)]
    fn cursor_pos(&self) -> (i32, i32) {
        let mut di1 = 0;
        let mut di2 = 0;
        let mut dui = 0;
        let mut win1 = 0;
        let mut win2 = 0;
        let mut x = 0;
        let mut y = 0;

        unsafe {
            xlib::XQueryPointer(
                self.dpy(),
                self.root(),
                &mut win1,
                &mut win2,
                &mut x,
                &mut y,
                &mut di1,
                &mut di2,
                &mut dui,
            )
        };

        (x, y)
    }

    // mod1mask + mousebutton1 moves window
    fn handle_move_window(&mut self, event: &xlib::XEvent) {
        let clean_mask = self.clean_mask();

        if unsafe {
            self.move_window.is_none()
                && event.get_type() == xlib::ButtonPress
                && event.button.button == 1
                && event.button.state & clean_mask == Mod1Mask & clean_mask
                && event.button.subwindow != 0
        } {
            let win_pos = unsafe {
                let mut attr: xlib::XWindowAttributes =
                    std::mem::MaybeUninit::uninit().assume_init();
                xlib::XGetWindowAttributes(self.dpy(), event.button.subwindow, &mut attr);

                (attr.x, attr.y)
            };

            self.move_window = unsafe {
                Some((
                    event.button.subwindow,
                    (event.button.x, event.button.y),
                    win_pos,
                ))
            };
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
                    std::mem::MaybeUninit::uninit().assume_init();
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
                    self.move_window.unwrap().0,
                    (xlib::CWX | xlib::CWY) as u32,
                    &mut wc,
                );

				xlib::XSync(self.dpy(), 0);
            }
        }
    }

    // mod1mask + mousebutton3 resizes window
    fn handle_resize_window(&mut self, event: &xlib::XEvent) {
        let clean_mask = self.clean_mask();

        if unsafe {
            self.resize_window.is_none()
                && event.get_type() == xlib::ButtonPress
                && event.button.button == 3
                && event.button.state & clean_mask == Mod1Mask & clean_mask
                && event.button.subwindow != 0
        } {
            unsafe {
                let mut attr: xlib::XWindowAttributes =
                    std::mem::MaybeUninit::uninit().assume_init();

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
        } else if unsafe {
            self.resize_window.is_some()
                && event.get_type() == xlib::ButtonRelease
                && event.button.button == 3
        } {
            self.resize_window = None;
        } else if self.resize_window.is_some() && event.get_type() == xlib::MotionNotify {
            let resize_window = self.resize_window.unwrap();

            let attr = unsafe {
                let mut attr: xlib::XWindowAttributes =
                    std::mem::MaybeUninit::uninit().assume_init();
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

				/*
                xlib::XWarpPointer(
                    self.dpy(),
                    0,
                    resize_window.0,
                    0,
                    0,
                    0,
                    0,
                    width - 1,
                    height - 1,
                );
				*/

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

                //xlib::XFlush(self.dpy());
				xlib::XSync(self.dpy(), 0);
            }
        }
    }

    fn main_loop(mut self) -> Result<Self> {
        loop {
            let event = unsafe {
                let mut event: xlib::XEvent = std::mem::MaybeUninit::uninit().assume_init();
                xlib::XNextEvent(self.dpy(), &mut event);

                event
            };

            // run keypress handlers
            if event.get_type() == xlib::KeyPress {
                // cache clean mask, that way numlock_mask doesnt get called for every cmp
                let clean_mask = self.clean_mask();

                for (key, mask, handler) in self.keys.iter() {
                    // check if key and mask with any numlock state fit
                    if unsafe {
                        event.key.keycode == *key as u32
                            && event.key.state & clean_mask == *mask & clean_mask
                    } {
                        handler(&self, &event);
                    }
                }
            }

            // handle window resizes
            self.handle_move_window(&event);
            self.handle_resize_window(&event);

            /*
            else if event.get_type() == xlib::ButtonPress && event.button.subwindow != 0 {
                xlib::XGetWindowAttributes(state.dpy(), event.button.subwindow, &mut attr);
                start = event.button;
            }
            else if event.get_type() == xlib::MotionNotify && start.subwindow != 0 {
                let xdiff = event.button.x_root - start.x_root;
                let ydiff = event.button.y_root - start.y_root;

                xlib::XMoveResizeWindow(state.dpy(),
                                        start.subwindow,
                                        attr.x + if start.button == 1 { xdiff } else { 0 },
                                        attr.y + if start.button == 1 { ydiff } else { 0 },
                                        std::cmp::max(1, attr.width +
                                                      if start.button == 3 { xdiff }
                                                      else { 0 }) as u32,
                                        std::cmp::max(1, attr.height +
                                                      if start.button == 3 { ydiff }
                                                      else { 0 }) as u32);
            }
            else if event.get_type() == xlib::ButtonRelease {
                start.subwindow = 0;
            }
             */
        }
    }

    fn add_key_with_handler<S: Into<String>>(
        mut self,
        key: S,
        mask: u32,
        handler: Box<dyn Fn(&Self, &XEvent)>,
    ) -> Self {
        let keycode = self.keycode(key);

        self.keys.push((keycode, mask, Box::new(handler)));
        self.grab_key(keycode, mask);

        self
    }

    fn grab_key(&self, keycode: i32, mask: u32) -> &Self {
        let numlock_mask = self.numlock_mask();
        let modifiers = vec![0, LockMask, numlock_mask, LockMask | numlock_mask];
        for &modifier in modifiers.iter() {
            unsafe {
                xlib::XGrabKey(
                    self.dpy(),
                    keycode,
                    mask | modifier,
                    self.root(),
                    1, /* true */
                    GrabModeAsync,
                    GrabModeAsync,
                );
            }
        }

        self
    }

    fn grab_button(&self, button: u32, mod_mask: u32, button_mask: i64) -> &Self {
        let numlock_mask = self.numlock_mask();
        let modifiers = vec![0, LockMask, numlock_mask, LockMask | numlock_mask];

        for &modifier in modifiers.iter() {
            unsafe {
                xlib::XGrabButton(
                    self.dpy(),
                    button,
                    mod_mask | modifier,
                    self.root(),
                    1, /*true */
                    button_mask as u32,
                    GrabModeAsync,
                    GrabModeAsync,
                    0,
                    0,
                );
            }
        }

        self
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

    fn keycode<S: Into<String>>(&self, string: S) -> i32 {
        let c_string = CString::new(string.into()).unwrap();
        unsafe {
            let keysym = xlib::XStringToKeysym(c_string.as_ptr());
            xlib::XKeysymToKeycode(self.dpy(), keysym) as i32
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

fn main() -> Result<()> {
    println!("Hello, world!");

    let state = XlibState::new()?;

    let state = state
        .add_key_with_handler(
            "T",
            Mod1Mask,
            Box::new(|state, _| {
                let _ = state.spawn("xterm", &[]);
            }),
        )
        .add_key_with_handler(
            "F1",
            Mod1Mask,
            Box::new(|state, event| unsafe {
                if event.key.subwindow != 0 {
                    xlib::XRaiseWindow(state.dpy(), event.key.subwindow);
                }
            }),
        );

    state
        .grab_key(state.keycode("F1"), Mod1Mask)
        .grab_button(
            1,
            Mod1Mask,
            ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
        )
        .grab_button(
            3,
            Mod1Mask,
            ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
        );

    state.main_loop()?;

    Ok(())
}
