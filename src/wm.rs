// asdf
use std::{
	cell::{RefCell, RefMut},
	collections::{hash_map::Entry, HashMap},
	ffi::CString,
	io::{Error, ErrorKind, Result},
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
	pub protocols: Option<Atom>,
	pub delete: Option<Atom>,
	pub active_window: Option<Atom>,
	pub take_focus: Option<Atom>,
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
			active_window: {
				Some(unsafe {
					let atom_cstr = CString::new("_NET_ACTIVE_WINDOW").unwrap();
					XInternAtom(display.get(), atom_cstr.as_c_str().as_ptr(), 0)
				})
				.filter(|&atom| atom != 0)
			},
			take_focus: {
				Some(unsafe {
					let atom_cstr = CString::new("WM_TAKE_FOCUS").unwrap();
					XInternAtom(display.get(), atom_cstr.as_c_str().as_ptr(), 0)
				})
				.filter(|&atom| atom != 0)
			},
			..Default::default()
		}
	}
}

impl Default for WMAtoms {
	fn default() -> Self {
		Self {
			protocols: None,
			delete: None,
			active_window: None,
			take_focus: None,
		}
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

use std::hash::{BuildHasherDefault, Hasher};

#[derive(Debug, Clone, Copy, Default)]
struct IdentityHasher(usize);

impl Hasher for IdentityHasher {
	fn finish(&self) -> u64 {
		self.0 as u64
	}

	fn write(&mut self, _bytes: &[u8]) {
		unimplemented!("IdentityHasher only supports usize keys")
	}

	fn write_u64(&mut self, i: u64) {
		self.0 = i as usize;
	}

	fn write_usize(&mut self, i: usize) {
		self.0 = i;
	}
}

type BuildIdentityHasher = BuildHasherDefault<IdentityHasher>;

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

	fn contains_window(&self, window: &Window) -> bool {
		self.master_stack.contains_key(window) || self.aux_stack.contains_key(window)
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

	fn send_event(&self, window: xlib::Window, proto: Option<xlib::Atom>) -> bool {
		if proto.is_some()
			&& self.check_for_protocol(window, proto.unwrap())
			&& self.atoms.protocols.is_some()
		{
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

#[derive(Clone, Debug)]
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
	clients: HashMap<Window, Rc<RefCell<Client>>>,
	focused_client: Weak<RefCell<Client>>,
	current_vscreen: usize,
	virtual_screens: Vec<VirtualScreen>,
}

impl Default for WMStateMut {
	fn default() -> Self {
		Self {
			move_window: None,
			resize_window: None,
			clients: HashMap::new(),
			focused_client: Weak::new(),
			current_vscreen: 0,
			virtual_screens: vec![VirtualScreen::new()],
		}
	}
}

impl WMStateMut {
	fn stack_unstacked_clients(&mut self) {
		info!("[stack_unstacked_clients] ");
		let current_vscreen = self.current_vscreen;

		self.clients
			.iter()
			.filter(|(w, c)| {
				!c.borrow().floating && self.virtual_screens.iter().any(|vs| !vs.contains_window(w))
			})
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
					self.virtual_screens[current_vscreen]
						.aux_stack
						.remove(&w)
				});
		}
	}
}

pub struct WMState {
	xlib_state: XLibState,
	key_handlers: Vec<(i32, u32, Rc<dyn Fn(&Self, &XEvent)>)>,
	// (button, mod_mask, button_mask)
	buttons: Vec<(u32, u32, i64)>,
	event_handlers: Vec<Rc<dyn Fn(&Self, &XEvent)>>,
	mut_state: RefCell<WMStateMut>,
}

impl WMState {
	pub fn new() -> Self {
		Self {
			xlib_state: XLibState::new(),
			mut_state: RefCell::new(WMStateMut::default()),
			key_handlers: vec![],
			event_handlers: vec![],
			buttons: vec![],
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
			.add_key_handler("M", Mod1Mask, Self::handle_switch_stack)
			.add_key_handler("Q", Mod1Mask, |state, event| unsafe {
				if event.key.subwindow != 0 {
					if state.xlib_state.atoms.delete.is_none()
						|| !state
							.xlib_state
							.send_event(event.key.subwindow, state.xlib_state.atoms.delete)
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

	pub fn run(self) -> Self {
		loop {
			let event = unsafe {
				let mut event: xlib::XEvent = std::mem::MaybeUninit::zeroed().assume_init();
				xlib::XNextEvent(self.dpy(), &mut event);

				event
			};

			self.event_handlers.iter().for_each(|handler| {
				handler(&self, &event);
			});

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

					self.key_handlers.iter().for_each(|(key, mask, handler)| {
						if unsafe {
							event.key.keycode == *key as u32
								&& event.key.state & clean_mask == *mask & clean_mask
						} {
							handler(&self, &event);
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
		F: Fn(&Self, &XEvent) + 'static,
	{
		self.event_handlers.push(Rc::new(handler));

		self
	}

	pub fn add_key_handler<S, F>(mut self, key: S, mask: u32, handler: F) -> Self
	where
		S: Into<String>,
		F: Fn(&Self, &XEvent) + 'static,
	{
		let keycode = self.xlib_state.keycode(key);

		self.key_handlers.push((keycode, mask, Rc::new(handler)));
		self.xlib_state.grab_key(self.root(), keycode, mask);

		self
	}

	fn map_request(&self, event: &xlib::XMapRequestEvent) {
		info!("[MapRequest] event: {:#?}", event);

		let _ = Some(self.mut_state.borrow_mut())
			.and_then(|mut state| {
				if !state.clients.contains_key(&event.window) {
					info!("[MapRequest] new client: {:#?}", event.window);
					Some(
						state
							.clients
							.entry(event.window)
							.or_insert_with(|| Rc::new(RefCell::new(Client::new(event.window))))
							.clone(),
					)
				} else {
					None
				}
			})
			.and_then(|c| {
				unsafe {
					xlib::XMapWindow(self.dpy(), c.borrow().window);

					xlib::XSelectInput(
						self.dpy(),
						event.window,
						EnterWindowMask
							| FocusChangeMask | PropertyChangeMask
							| StructureNotifyMask,
					);
				}

				self.buttons
					.iter()
					.for_each(|&(button, mod_mask, button_mask)| {
						self.xlib_state.grab_button(
							c.borrow().window,
							button,
							mod_mask,
							button_mask,
						);
					});

				self.arrange_clients();
				self.focus_client(c.clone());

				Some(())
			});
	}

	fn unmap_notify(&self, event: &xlib::XUnmapEvent) {
		info!("[UnmapNotify] event: {:#?}", event);

		if event.send_event == 0 {
			let _ = Some(self.mut_state.borrow_mut()).and_then(|mut state| {
				if state.clients.contains_key(&event.window) {
					let client = state.clients.remove(&event.window);
					info!("[UnmapNotify] removing client: {:#?}", client);
				}

				Some(())
			});
		}

		self.arrange_clients();
	}

	fn destroy_notify(&self, event: &xlib::XDestroyWindowEvent) {
		info!("[DestroyNotify] event: {:?}", event);

		let _ = Some(self.mut_state.borrow_mut()).and_then(|mut state| {
			let _entry = state.clients.remove(&event.window);

			info!("[DestroyNotify] removed entry: {:?}", _entry);

			Some(())
		});

		self.arrange_clients();
	}

	fn configure_request(&self, event: &xlib::XConfigureRequestEvent) {
		info!("[ConfigureRequest] event: {:?}", event);

		match self.mut_state.borrow_mut().clients.entry(event.window) {
			Entry::Occupied(entry) => {
				info!(
					"[ConfigureRequest] found Client for Window({:?}): {:#?}",
					event.window,
					entry.get()
				);

				self.configure_client(entry.get().clone());
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

	fn enter_notify(&self, event: &xlib::XCrossingEvent) {
		info!("[EnterNotify] event: {:?}", event);

		Some(self.mut_state.borrow())
			.and_then(|state| {
				state
					.clients
					.get(&event.window)
					.and_then(|c| Some(c.clone()))
			})
			.and_then(|c| {
				info!(
					"[EnterNotify] focusing Client for Window({:?})",
					event.window
				);
				self.focus_client(c);

				Some(())
			});
	}

	fn button_press(&self, event: &xlib::XButtonEvent) {
		info!("[ButtonPress] event: {:?}", event);

		Some(self.mut_state.borrow())
			.and_then(|state| {
				state
					.clients
					.get(&event.subwindow)
					.and_then(|c| Some(c.clone()))
			})
			.and_then(|c| {
				info!(
					"[ButtonPress] focusing Client for Window({:?})",
					event.window
				);

				self.focus_client(c.clone());

				info!("[ButtonPress] raising Window({:?})", event.window);

				unsafe {
					xlib::XRaiseWindow(self.dpy(), c.borrow().window);
					xlib::XSync(self.dpy(), 0);
				}

				Some(())
			});
	}

	fn unfocus_client(&self, client: Rc<RefCell<Client>>) {
		unsafe {
			xlib::XSetInputFocus(
				self.dpy(),
				client.borrow().window,
				xlib::RevertToPointerRoot,
				xlib::CurrentTime,
			);

			xlib::XDeleteProperty(
				self.dpy(),
				self.root(),
				self.xlib_state.atoms.active_window.unwrap(),
			);
		}
	}

	fn focus_client(&self, client: Rc<RefCell<Client>>) {
		Some(self.mut_state.borrow_mut()).and_then(|mut state| {
			let current_vscreen = state.current_vscreen;

			state
				.focused_client
				.upgrade()
				.and_then(|c| Some(self.unfocus_client(c)));

			state.focused_client = Rc::downgrade(&client);

			state.virtual_screens[current_vscreen].focused_client = state.focused_client.clone();

			Some(())
		});

		unsafe {
			xlib::XSetInputFocus(
				self.dpy(),
				client.borrow().window,
				xlib::RevertToPointerRoot,
				xlib::CurrentTime,
			);

			xlib::XChangeProperty(
				self.dpy(),
				self.root(),
				self.xlib_state.atoms.active_window.unwrap(),
				xlib::XA_WINDOW,
				32,
				xlib::PropModeReplace,
				&client.borrow().window as *const u64 as *const _,
				1,
			);
		}

		self.xlib_state
			.send_event(client.borrow().window, self.xlib_state.atoms.take_focus);
	}

	fn arrange_clients(&self) {
		let (screen_w, screen_h) = unsafe {
			(
				xlib::XDisplayWidth(self.dpy(), self.xlib_state.screen()),
				xlib::XDisplayHeight(self.dpy(), self.xlib_state.screen()),
			)
		};

		Some(self.mut_state.borrow_mut()).and_then(|mut state| {
			if !state.clients.is_empty() {
				info!("[arrange_clients] refreshing screen");
				state.refresh_screen();
				info!("[arrange_clients] refreshing screen");

				let window_w = {
					if state.virtual_screens[state.current_vscreen]
						.aux_stack
						.is_empty()
					{
						screen_w
					} else {
						screen_w / 2
					}
				};

				for (i, (_, weak_client)) in state.virtual_screens[state.current_vscreen]
					.master_stack
					.iter()
					.enumerate()
				{
					let client = weak_client.upgrade().unwrap();

					let mut wc = {
						let mut client = client.borrow_mut();
						let window_h = screen_h
							/ state.virtual_screens[state.current_vscreen]
								.master_stack
								.len() as i32;

						client.size = (window_w, window_h);
						client.position = (0, window_h * i as i32);

						xlib::XWindowChanges {
							x: client.position.0,
							y: client.position.1,
							width: client.size.0,
							height: client.size.1,
							border_width: 0,
							sibling: 0,
							stack_mode: 0,
						}
					};

					unsafe {
						xlib::XConfigureWindow(
							self.dpy(),
							client.borrow().window,
							(xlib::CWY | xlib::CWX | xlib::CWHeight | xlib::CWWidth) as u32,
							&mut wc,
						);

						self.configure_client(client.clone());

						xlib::XSync(self.dpy(), 0);
					}
				}

				for (i, (_, weak_client)) in state.virtual_screens[state.current_vscreen]
					.aux_stack
					.iter()
					.enumerate()
				{
					let client = weak_client.upgrade().unwrap();

					let mut wc = {
						let mut client = client.borrow_mut();
						let window_h = screen_h
							/ state.virtual_screens[state.current_vscreen].aux_stack.len() as i32;

						client.size = (window_w, window_h);
						client.position = (window_w, window_h * i as i32);

						xlib::XWindowChanges {
							x: client.position.0,
							y: client.position.1,
							width: client.size.0,
							height: client.size.1,
							border_width: 0,
							sibling: 0,
							stack_mode: 0,
						}
					};

					unsafe {
						xlib::XConfigureWindow(
							self.dpy(),
							client.borrow().window,
							(xlib::CWY | xlib::CWX | xlib::CWHeight | xlib::CWWidth) as u32,
							&mut wc,
						);

						self.configure_client(client.clone());

						xlib::XSync(self.dpy(), 0);
					}
				}
			}

			Some(())
		});
	}

	fn configure_client(&self, client: Rc<RefCell<Client>>) {
		let mut event = {
			let client = client.borrow();

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

	fn handle_switch_stack(&self, event: &XEvent) {
		self.mut_state
			.borrow_mut()
			.switch_stack_for_client(unsafe { &event.button.subwindow });

		self.arrange_clients();
	}

	fn handle_toggle_floating(&self, event: &XEvent) {
		if event.get_type() == xlib::ButtonPress {
			let event = unsafe { event.button };
			let clean_mask = self.xlib_state.clean_mask();

			if event.button == 2
				&& event.state & clean_mask == Mod1Mask & clean_mask
				&& event.subwindow != 0
			{
				self.mut_state
					.borrow_mut()
					.clients
					.entry(event.subwindow)
					.and_modify(|c| {
						let mut c = c.borrow_mut();
						info!(
							"[handle_toggle_floating] {:#?} floating -> {:?}",
							c, !c.floating
						);
						c.floating = !c.floating;
					});

				self.arrange_clients();
			}
		}
	}

	fn handle_move_window(&self, event: &XEvent) {
		let clean_mask = self.xlib_state.clean_mask();

		let move_window = self.mut_state.borrow_mut().move_window;

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

			self.mut_state.borrow_mut().move_window = Some(unsafe {
				(
					event.button.subwindow,
					(event.button.x, event.button.y),
					win_pos,
				)
			});

			self.mut_state
				.borrow_mut()
				.clients
				.entry(unsafe { event.button.subwindow })
				.and_modify(|c| c.borrow_mut().floating = true);

			self.arrange_clients();
		} else if unsafe {
			move_window.is_some()
				&& event.get_type() == xlib::ButtonRelease
				&& event.button.button == 1
		} {
			self.mut_state.borrow_mut().move_window = None;
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

		let resize_window = self.mut_state.borrow().resize_window;

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

				self.mut_state.borrow_mut().resize_window =
					Some((event.button.subwindow, (attr.x, attr.y)));

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

			self.mut_state
				.borrow_mut()
				.clients
				.entry(unsafe { event.button.subwindow })
				.and_modify(|c| c.borrow_mut().floating = true);

			self.arrange_clients();
		} else if unsafe {
			resize_window.is_some()
				&& event.get_type() == xlib::ButtonRelease
				&& event.button.button == 3
		} {
			self.mut_state.borrow_mut().resize_window = None;
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
