use x11::xlib;

use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::ffi::CString;
use std::io::{Result /*, Error, ErrorKind */};
use std::sync::atomic::{AtomicPtr, Ordering};


type Display = Arc<AtomicPtr<xlib::Display>>;

#[derive(Debug)]
struct XlibState {
	display: Display,
}

impl XlibState {
	fn new() -> Result<Self> {
		let display = unsafe { xlib::XOpenDisplay(null()) };
		assert_ne!(display, null_mut());

		let display = Display::new(AtomicPtr::new(display));

		Ok(Self {
			display,
		})
	}

	fn dpy(&self) -> *mut xlib::Display {
		self.display.load(Ordering::SeqCst)
	}

	fn root(&self) -> u64 {
		unsafe { xlib::XDefaultRootWindow(self.dpy()) } 
	}

	fn keycode<S: Into<String>>(&self, string: S) -> i32 {
		let c_string = CString::new(string.into()).unwrap();
		unsafe {
			let keysym = xlib::XStringToKeysym(c_string.as_ptr());
			xlib::XKeysymToKeycode(self.dpy(), keysym) as i32
		}
	}
}

use x11::xlib::{ButtonPressMask, ButtonReleaseMask, PointerMotionMask, GrabModeAsync, Mod1Mask};

fn main() -> Result<()> {
    println!("Hello, world!");

	let state = XlibState::new()?;

	unsafe {
		xlib::XGrabKey(state.dpy(),
					   state.keycode("F1"),
					   Mod1Mask,
					   state.root(),
					   1 /* true */,
					   GrabModeAsync,
					   GrabModeAsync);

		xlib::XGrabButton(state.dpy(),
						  1,
						  Mod1Mask,
						  state.root(),
						  1 /*true */,
						  (ButtonPressMask | ButtonReleaseMask | PointerMotionMask) as u32,
						  GrabModeAsync, GrabModeAsync, 0, 0);

		xlib::XGrabButton(state.dpy(),
						  3,
						  Mod1Mask,
						  state.root(),
						  1 /*true */,
						  (ButtonPressMask | ButtonReleaseMask | PointerMotionMask) as u32,
						  GrabModeAsync, GrabModeAsync, 0, 0);
	}

	let mut attr: xlib::XWindowAttributes = unsafe { std::mem::MaybeUninit::uninit().assume_init() };
	let mut start: xlib::XButtonEvent = unsafe { std::mem::MaybeUninit::uninit().assume_init() };

	loop {


		unsafe {
			let mut event: xlib::XEvent = std::mem::MaybeUninit::uninit().assume_init();
			xlib::XNextEvent(state.dpy(), &mut event);

			if event.get_type() == xlib::KeyPress && event.key.subwindow != 0 {
				xlib::XRaiseWindow(state.dpy(), event.key.subwindow);
			}
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
		}

	}
}
