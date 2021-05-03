use std::rc::Rc;

use log::{error, info};

use x11::xlib::{
    self, ShiftMask, Window, XButtonEvent, XButtonPressedEvent, XButtonReleasedEvent, XEvent,
    XKeyEvent, XMotionEvent,
};
use xlib::{
    ButtonPressMask, ButtonReleaseMask, Mod1Mask, PointerMotionMask, XConfigureRequestEvent,
    XCrossingEvent, XDestroyWindowEvent, XMapRequestEvent, XUnmapEvent,
};

use crate::{
    clients::{Client, ClientKey, ClientState},
    xlib::KeyOrButton,
    xlib::XLib,
};

pub struct WindowManager {
    clients: ClientState,
    move_resize_window: MoveResizeInfo,
    keybinds: Vec<KeyBinding>,
    xlib: XLib,
}

#[derive(Debug)]
pub enum Direction {
    Left,
    Right,
}

enum MoveResizeInfo {
    Move(MoveInfoInner),
    Resize(ResizeInfoInner),
    None,
}

struct MoveInfoInner {
    window: Window,
    starting_cursor_pos: (i32, i32),
    starting_window_pos: (i32, i32),
}

struct ResizeInfoInner {
    window: Window,
    starting_cursor_pos: (i32, i32),
    starting_window_size: (i32, i32),
}

#[derive(Clone)]
struct KeyBinding {
    key: KeyOrButton,
    closure: Rc<dyn Fn(&mut WindowManager, &XKeyEvent)>,
}

impl WindowManager {
    pub fn new() -> Self {
        let clients = ClientState::with_virtualscreens(3);
        let xlib = XLib::new();

        Self {
            clients,
            move_resize_window: MoveResizeInfo::None,
            keybinds: Vec::new(),
            xlib,
        }
    }

    pub fn init(mut self) -> Self {
        self.xlib.add_global_keybind(KeyOrButton::button(
            1,
            Mod1Mask,
            ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
        ));
        self.xlib.add_global_keybind(KeyOrButton::button(
            2,
            Mod1Mask,
            ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
        ));
        self.xlib.add_global_keybind(KeyOrButton::button(
            3,
            Mod1Mask,
            ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("P", Mod1Mask),
            |wm, _| {
                wm.spawn(
                    "dmenu_run",
                    &[
                        "-m",
                        "0",
                        "-fn",
                        "'New York:size=13'",
                        "-nb",
                        "#222222",
                        "-nf",
                        "#bbbbbb",
                        "-sb",
                        "#dddddd",
                        "-sf",
                        "#eeeeee",
                    ],
                )
            },
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("M", Mod1Mask),
            Self::handle_switch_stack,
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("Q", Mod1Mask),
            Self::kill_client,
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("Q", Mod1Mask | ShiftMask),
            |wm, _| wm.quit(),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("T", Mod1Mask),
            |wm, _| wm.spawn("alacritty", &[]),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("Return", Mod1Mask | ShiftMask),
            |wm, _| wm.spawn("alacritty", &[]),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("Left", Mod1Mask),
            |wm, _| wm.rotate_virtual_screen(Direction::Left),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("Right", Mod1Mask),
            |wm, _| wm.rotate_virtual_screen(Direction::Right),
        ));

        self.xlib.init();

        self
    }

    fn add_keybind(&mut self, keybind: KeyBinding) {
        self.xlib.add_global_keybind(keybind.key);
        self.keybinds.push(keybind);
    }

    pub fn run(mut self) -> ! {
        loop {
            let event = self.xlib.next_event();

            match event.get_type() {
                xlib::MapRequest => self.map_request(&event),
                xlib::UnmapNotify => self.unmap_notify(&event),
                xlib::ConfigureRequest => self.configure_request(&event),
                xlib::EnterNotify => self.enter_notify(&event),
                xlib::DestroyNotify => self.destroy_notify(&event),
                xlib::ButtonPress => self.button_press(event.as_ref()),
                xlib::ButtonRelease => self.button_release(event.as_ref()),
                xlib::MotionNotify => self.motion_notify(event.as_ref()),
                xlib::KeyPress => self.handle_keybinds(event.as_ref()),
                _ => {}
            }
        }
    }

    fn quit(&self) -> ! {
        self.xlib.close_dpy();

        info!("Goodbye.");

        std::process::exit(0);
    }

    fn kill_client(&mut self, event: &XKeyEvent) {
        if let Some(client) = self.clients.get(&event.subwindow).into_option() {
            self.xlib.kill_client(client);
        }
    }

    // TODO: change this somehow cuz I'm not a big fan of this "hardcoded" keybind stuff
    fn handle_keybinds(&mut self, event: &XKeyEvent) {
        let clean_mask = self.xlib.get_clean_mask();
        for kb in self.keybinds.clone().into_iter() {
            if let KeyOrButton::Key(keycode, modmask) = kb.key {
                if keycode as u32 == event.keycode
                    && modmask & clean_mask == event.state & clean_mask
                {
                    (kb.closure)(self, event);
                }
            }
        }
    }

    fn handle_toggle_floating(&mut self, event: &XEvent) {
        if event.get_type() == xlib::ButtonPress {
            let event: &XButtonEvent = event.as_ref();
            let clean_mask = self.xlib.get_clean_mask();

            if event.button == 2 && event.state & clean_mask == Mod1Mask & clean_mask {
                if self.clients.contains(&event.subwindow) {
                    info!("toggleing floating for {:?}", event.subwindow);

                    self.clients.toggle_floating(&event.subwindow);

                    self.arrange_clients();
                }
            }
        }
    }

    fn handle_switch_stack(&mut self, event: &XKeyEvent) {
        info!("Switching stack for window{:?}", event.subwindow);

        self.clients.switch_stack_for_client(&event.subwindow);

        self.arrange_clients();
    }

    fn rotate_virtual_screen(&mut self, dir: Direction) {
        info!("rotateing VS: {:?}", dir);

        match dir {
            Direction::Left => self.clients.rotate_left(),
            Direction::Right => self.clients.rotate_right(),
        }

        self.arrange_clients();

        // focus first client in all visible clients
        let to_focus = self.clients.iter_visible().next().map(|(k, _)| k).cloned();

        if let Some(key) = to_focus {
            self.focus_client(&key);
        }
    }

    fn hide_hidden_clients(&self) {
        self.clients
            .iter_hidden()
            .for_each(|(_, c)| self.xlib.hide_client(c));
    }

    fn raise_floating_clients(&self) {
        self.clients
            .iter_floating()
            .for_each(|(_, c)| self.xlib.raise_client(c));

        self.clients
            .iter_transient()
            .for_each(|(_, c)| self.xlib.raise_client(c));
    }

    fn arrange_clients(&mut self) {
        let (width, height) = self.xlib.dimensions();
        self.clients.arrange_virtual_screen(width, height, Some(2));

        self.hide_hidden_clients();

        self.clients
            .iter_visible()
            .for_each(|(_, c)| self.xlib.move_resize_client(c));

        self.raise_floating_clients();
    }

    fn focus_client<K>(&mut self, key: &K)
    where
        K: ClientKey,
    {
        let (new, old) = self.clients.focus_client(key);

        if let Some(old) = old.into_option() {
            self.xlib.unfocus_client(old);
        }

        if let Some(new) = new.into_option() {
            self.xlib.focus_client(new);
            self.xlib.raise_client(new);
        }

        self.raise_floating_clients();
    }

    fn new_client(&mut self, window: Window) {
        info!("new client: {:?}", window);
        let client = if let Some(transient_window) = self.xlib.get_transient_for_window(window) {
            Client::new_transient(
                window,
                self.xlib.get_window_size(window).unwrap_or((100, 100)),
                transient_window,
            )
        } else {
            Client::new_default(window)
        };

        self.clients.insert(client).unwrap();
        self.xlib.map_window(window);

        self.focus_client(&window);

        self.arrange_clients();
    }

    fn map_request(&mut self, event: &XEvent) {
        let event: &XMapRequestEvent = event.as_ref();

        if !self.clients.contains(&event.window) {
            info!("MapRequest: new client: {:?}", event.window);

            self.new_client(event.window);
        }
    }

    fn unmap_notify(&mut self, event: &XEvent) {
        let event: &XUnmapEvent = event.as_ref();
        info!("unmap_notify: {:?}", event.window);

        self.clients.remove(&event.window);

        self.arrange_clients();
    }

    fn destroy_notify(&mut self, event: &XEvent) {
        let event: &XDestroyWindowEvent = event.as_ref();
        info!("destroy_notify: {:?}", event.window);

        self.clients.remove(&event.window);

        self.arrange_clients();
    }

    fn configure_request(&mut self, event: &XEvent) {
        let event: &XConfigureRequestEvent = event.as_ref();

        match self.clients.get(&event.window).into_option() {
            Some(client) => self.xlib.configure_client(client),
            None => self.xlib.configure_window(event),
        }
    }

    fn enter_notify(&mut self, event: &XEvent) {
        let event: &XCrossingEvent = event.as_ref();

        self.focus_client(&event.window);
    }

    /// ensure event.subwindow refers to a valid client.
    fn start_move_resize_window(&mut self, event: &XButtonPressedEvent) {
        let window = event.subwindow;

        match event.button {
            1 => {
                if self.clients.set_floating(&window) {
                    self.arrange_clients();
                }

                self.move_resize_window = MoveResizeInfo::Move(MoveInfoInner {
                    window,
                    starting_cursor_pos: (event.x, event.y),
                    starting_window_pos: self.clients.get(&window).unwrap().position,
                });
            }
            3 => {
                if self.clients.set_floating(&window) {
                    self.arrange_clients();
                }

                let client = self.clients.get(&window).unwrap();

                let corner_pos = {
                    (
                        client.position.0 + client.size.0,
                        client.position.1 + client.size.1,
                    )
                };

                self.xlib.move_cursor(None, corner_pos);
                self.xlib.grab_cursor();

                self.move_resize_window = MoveResizeInfo::Resize(ResizeInfoInner {
                    window,
                    starting_cursor_pos: corner_pos,
                    starting_window_size: client.size,
                });
            }
            _ => {}
        }
    }

    fn end_move_resize_window(&mut self, event: &XButtonReleasedEvent) {
        if event.button == 1 || event.button == 3 {
            self.move_resize_window = MoveResizeInfo::None;
        }
        if event.button == 3 {
            self.xlib.release_cursor();
        }
    }

    fn do_move_resize_window(&mut self, event: &XMotionEvent) {
        match &self.move_resize_window {
            MoveResizeInfo::Move(info) => {
                let (x, y) = (
                    event.x - info.starting_cursor_pos.0,
                    event.y - info.starting_cursor_pos.1,
                );

                if let Some(client) = self.clients.get_mut(&info.window).into_option() {
                    let position = &mut client.position;

                    position.0 = info.starting_window_pos.0 + x;
                    position.1 = info.starting_window_pos.1 + y;

                    self.xlib.move_client(client);
                }
            }
            MoveResizeInfo::Resize(info) => {
                let (x, y) = (
                    event.x - info.starting_cursor_pos.0,
                    event.y - info.starting_cursor_pos.1,
                );

                if let Some(client) = self.clients.get_mut(&info.window).into_option() {
                    let size = &mut client.size;

                    size.0 = std::cmp::max(1, info.starting_window_size.0 + x);
                    size.1 = std::cmp::max(1, info.starting_window_size.1 + y);

                    self.xlib.resize_client(client);
                }
            }
            _ => {}
        }
    }

    fn button_press(&mut self, event: &XButtonPressedEvent) {
        self.focus_client(&event.subwindow);

        match event.button {
            1 | 3 => match self.move_resize_window {
                MoveResizeInfo::None
                    if self.xlib.are_masks_equal(event.state, Mod1Mask)
                        && self.clients.contains(&event.subwindow) =>
                {
                    self.start_move_resize_window(event)
                }
                _ => {}
            },
            2 => {
                self.clients.toggle_floating(&event.subwindow);
                self.arrange_clients();
            }
            _ => {}
        }
    }

    fn button_release(&mut self, event: &XButtonReleasedEvent) {
        match self.move_resize_window {
            MoveResizeInfo::None => {}
            _ => {
                self.end_move_resize_window(event);
            }
        }
    }

    fn motion_notify(&mut self, event: &XMotionEvent) {
        self.do_move_resize_window(event);
    }

    pub fn spawn(&self, command: &str, args: &[&str]) {
        info!("spawn: {:?} {:?}", command, args.join(" "));
        match std::process::Command::new(command).args(args).spawn() {
            Ok(_) => {}
            Err(err) => {
                error!("Failed to spawn {:?}: {:?}", command, err);
            }
        }
    }
}

impl KeyBinding {
    fn new<F>(key: KeyOrButton, closure: F) -> Self
    where
        F: Fn(&mut WindowManager, &XKeyEvent) + 'static,
    {
        Self {
            key,
            closure: Rc::new(closure),
        }
    }
}
