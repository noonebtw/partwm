use x11::xlib;

use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::ffi::CString;
use std::io::{Result, Error, ErrorKind};
use std::sync::atomic::{AtomicPtr, Ordering};

use x11::xlib::{ButtonPressMask, ButtonReleaseMask, PointerMotionMask,
				GrabModeAsync, XEvent};

use x11::xlib::{LockMask, ShiftMask, ControlMask, Mod1Mask, Mod2Mask, Mod3Mask, Mod4Mask, Mod5Mask};

use nix::unistd::{fork, ForkResult, close, setsid, execvp};


type Display = Arc<AtomicPtr<xlib::Display>>;

struct XlibState {
	display: Display,
	//buttons: Vec<(u32, u32, Box<dyn FnMut(&mut Self)>)>,
	keys: Vec<(i32, u32, Box<dyn Fn(&Self, &XEvent)>)>,
}

impl XlibState {
	fn new() -> Result<Self> {
		let display = unsafe { xlib::XOpenDisplay(null()) };
		assert_ne!(display, null_mut());

		let display = Display::new(AtomicPtr::new(display));

		Ok(Self {
			display,
			keys: vec![],
		})
	}

	fn dpy(&self) -> *mut xlib::Display {
		self.display.load(Ordering::SeqCst)
	}

	fn root(&self) -> u64 {
		unsafe { xlib::XDefaultRootWindow(self.dpy()) } 
	}

	fn add_key_with_handler<S: Into<String>>(mut self, key: S, mask: u32, handler: Box<dyn Fn(&Self, &XEvent)>)
								-> Self {
		let keycode = self.keycode(key);

		self.keys.push((keycode, mask, Box::new(handler)));
		self.grab_key(keycode, mask);

		self
	}

	fn grab_key(&self, keycode: i32, mask: u32) -> &Self {
		let numlock_mask = self.numlock_mask();
		let modifiers = vec![0, LockMask, numlock_mask, LockMask|numlock_mask];
		for &modifier in modifiers.iter() {
			unsafe {
				xlib::XGrabKey(self.dpy(),
							keycode,
							mask | modifier,
							self.root(),
							1 /* true */,
							GrabModeAsync,
							GrabModeAsync);
			}
		}

		self
	}

	fn grab_button(&self, button: u32, mod_mask: u32, button_mask: i64) -> &Self {
		let numlock_mask = self.numlock_mask();
		let modifiers = vec![0, LockMask, numlock_mask, LockMask|numlock_mask];

		for &modifier in modifiers.iter() {
			unsafe {
				xlib::XGrabButton(self.dpy(),
								  button,
								  mod_mask | modifier,
								  self.root(),
								  1 /*true */,
								  button_mask as u32,
								  GrabModeAsync, GrabModeAsync, 0, 0);
			}
		}

		self
	}

	// spawn a new process / calls execvp
	pub fn spawn<T: ToString>(&self, command: T, args: &[T]) -> Result<()> {
		let fd = unsafe {xlib::XConnectionNumber(self.dpy())};

		match unsafe { fork() } {
			Ok(ForkResult::Parent{..}) => {Ok(())},
			Ok(ForkResult::Child) => {
				// i dont think i want to exit this block without closing the program,
				// so unwrap everything

				close(fd).or_else(|_| Err("failed to close x connection")).unwrap();
				setsid().ok().ok_or("failed to setsid").unwrap();

				let c_cmd = CString::new(command.to_string()).unwrap();

				let c_args: Vec<_> = args.iter()
					.map(|s| CString::new(s.to_string()).unwrap())
					.collect();

				execvp(&c_cmd, &c_args.iter().map(|s| s.as_c_str()).collect::<Vec<_>>())
					.or(Err("failed to execvp()")).unwrap();

				eprintln!("execvp({}) failed.", c_cmd.to_str().unwrap());
				std::process::exit(0);
			},
			Err(_) => {Err(Error::new(ErrorKind::Other, "failed to fork."))},
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
					if *(*modmap).modifiermap.offset((i * max_keypermod + j) as isize) ==
						xlib::XKeysymToKeycode(self.dpy(), x11::keysym::XK_Num_Lock as u64) {
						return 1 << i;
					}
				}
			}
		}

		0
	}

	#[allow(non_snake_case)]
	fn clean_mask(&self) -> u32 {
		!(self.numlock_mask() | LockMask)
			& (ShiftMask|ControlMask|Mod1Mask|Mod2Mask|Mod3Mask|Mod4Mask|Mod5Mask)
	}
}


fn main() -> Result<()> {
    println!("Hello, world!");

	let state = XlibState::new()?;

	let state =
		state.add_key_with_handler("T", Mod1Mask, Box::new(|state, _| {
			let _ = state.spawn("xterm", &[]);
		}))
		.add_key_with_handler("F1", Mod1Mask, Box::new(|state, event| {
			unsafe {
				if event.key.subwindow != 0 {
					xlib::XRaiseWindow(state.dpy(), event.key.subwindow);
				}
			}
		}));

	state.grab_key(state.keycode("F1"), Mod1Mask)
		.grab_button(1, Mod1Mask, ButtonPressMask|ButtonReleaseMask|PointerMotionMask)
		.grab_button(3, Mod1Mask, ButtonPressMask|ButtonReleaseMask|PointerMotionMask);

	let mut attr: xlib::XWindowAttributes = unsafe { std::mem::MaybeUninit::uninit().assume_init() };
	let mut start: xlib::XButtonEvent = unsafe { std::mem::MaybeUninit::uninit().assume_init() };

	loop {


		unsafe {
			let mut event: xlib::XEvent = std::mem::MaybeUninit::uninit().assume_init();
			xlib::XNextEvent(state.dpy(), &mut event);

			// run keypress handlers
			if event.get_type() == xlib::KeyPress {

				// cache clean mask, that way numlock_mask doesnt get called for every cmp
				let clean_mask = state.clean_mask();

				for (key, mask, handler) in state.keys.iter() {
					// check if key and mask with any numlock state fit
					if event.key.keycode == *key as u32 &&
						event.key.state & clean_mask == *mask & clean_mask {
							handler(&state, &event);
					}
				}
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
