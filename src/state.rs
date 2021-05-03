use std::rc::Rc;

use log::{error, info};

use x11::xlib::{
    self, Mod4Mask, ShiftMask, Window, XButtonEvent, XEvent, XKeyEvent,
    XMotionEvent,
};
use xlib::{
    ButtonPressMask, ButtonReleaseMask, PointerMotionMask,
    XConfigureRequestEvent, XCrossingEvent, XDestroyWindowEvent,
    XMapRequestEvent, XUnmapEvent,
};

use crate::{
    clients::{Client, ClientKey, ClientState},
    xlib::KeyOrButton,
    xlib::XLib,
};

/**
Contains static config data for the window manager, the sort of stuff you might want to
be able to configure in a config file.
*/
pub struct WMConfig {
    num_virtualscreens: usize,
    mod_key: u32,
    gap: Option<i32>,
}

pub struct WindowManager {
    clients: ClientState,
    move_window: Option<MoveWindow>,
    resize_window: Option<MoveWindow>,
    keybinds: Vec<KeyBinding>,
    xlib: XLib,

    last_rotation: Option<Direction>,
    config: WMConfig,
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Left,
    Right,
}

struct MoveWindow {
    key: Window,
    cached_cursor_position: (i32, i32),
}

#[derive(Clone)]
struct KeyBinding {
    key: KeyOrButton,
    closure: Rc<dyn Fn(&mut WindowManager, &XKeyEvent)>,
}

impl WindowManager {
    pub fn new(config: WMConfig) -> Self {
        let clients =
            ClientState::with_virtualscreens(config.num_virtualscreens);
        let xlib = XLib::new();

        Self {
            clients,
            move_window: None,
            resize_window: None,
            keybinds: Vec::new(),
            xlib,
            last_rotation: None,
            config,
        }
    }

    pub fn init(mut self) -> Self {
        self.xlib.add_global_keybind(KeyOrButton::button(
            1,
            self.config.mod_key,
            ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
        ));
        self.xlib.add_global_keybind(KeyOrButton::button(
            2,
            self.config.mod_key,
            ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
        ));
        self.xlib.add_global_keybind(KeyOrButton::button(
            3,
            self.config.mod_key,
            ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("P", self.config.mod_key),
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
            self.xlib.make_key("M", self.config.mod_key),
            Self::handle_switch_stack,
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("Q", self.config.mod_key),
            Self::kill_client,
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("Q", self.config.mod_key | ShiftMask),
            |wm, _| wm.quit(),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("T", self.config.mod_key),
            |wm, _| wm.spawn("alacritty", &[]),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib
                .make_key("Return", self.config.mod_key | ShiftMask),
            |wm, _| wm.spawn("alacritty", &[]),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("Left", self.config.mod_key),
            |wm, _| wm.rotate_virtual_screen(Direction::Left),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("Right", self.config.mod_key),
            |wm, _| wm.rotate_virtual_screen(Direction::Right),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("Tab", self.config.mod_key),
            |wm, _| wm.rotate_virtual_screen_back(),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("J", self.config.mod_key),
            |wm, _| wm.focus_master_stack(),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("K", self.config.mod_key),
            |wm, _| wm.focus_aux_stack(),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("J", self.config.mod_key | ShiftMask),
            |wm, _| wm.rotate_virtual_screen(Direction::Left),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("K", self.config.mod_key | ShiftMask),
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

            self.handle_toggle_floating(&event);
            self.handle_move_window(&event);
            self.handle_resize_client(&event);

            match event.get_type() {
                xlib::MapRequest => self.map_request(&event),
                xlib::UnmapNotify => self.unmap_notify(&event),
                xlib::ConfigureRequest => self.configure_request(&event),
                xlib::EnterNotify => self.enter_notify(&event),
                xlib::DestroyNotify => self.destroy_notify(&event),
                xlib::ButtonPress => self.button_notify(&event),
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

            if event.button == 2
                && event.state & clean_mask == self.config.mod_key & clean_mask
            {
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

    fn handle_move_window(&mut self, event: &XEvent) {
        let clean_mask = self.xlib.get_clean_mask();

        match event.get_type() {
            xlib::ButtonPress => {
                let event: &XButtonEvent = event.as_ref();

                if self.move_window.is_none()
                    && event.button == 1
                    && event.state & clean_mask
                        == self.config.mod_key & clean_mask
                    && self.clients.contains(&event.subwindow)
                {
                    // if client is tiled, set to floating
                    if self.clients.set_floating(&event.subwindow) {
                        self.arrange_clients();
                    }

                    self.move_window = Some(MoveWindow {
                        key: event.subwindow,
                        cached_cursor_position: (event.x, event.y),
                    });
                }
            }

            // reset on release
            xlib::ButtonRelease => {
                let event: &XButtonEvent = event.as_ref();

                if event.button == 1 && self.move_window.is_some() {
                    self.move_window = None;
                }
            }

            xlib::MotionNotify => {
                //let event = self.xlib.squash_event(xlib::MotionNotify);
                let event: &XMotionEvent = event.as_ref();

                if let Some(move_window) = &mut self.move_window {
                    let (x, y) = (
                        event.x - move_window.cached_cursor_position.0,
                        event.y - move_window.cached_cursor_position.1,
                    );

                    move_window.cached_cursor_position = (event.x, event.y);

                    if let Some(client) =
                        self.clients.get_mut(&move_window.key).into_option()
                    {
                        let position = &mut client.position;
                        position.0 += x;
                        position.1 += y;

                        self.xlib.move_client(client);
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_resize_client(&mut self, event: &XEvent) {
        let clean_mask = self.xlib.get_clean_mask();

        match event.get_type() {
            xlib::ButtonPress => {
                let event: &XButtonEvent = event.as_ref();

                if self.resize_window.is_none()
                    && event.button == 3
                    && event.state & clean_mask
                        == self.config.mod_key & clean_mask
                    && self.clients.contains(&event.subwindow)
                {
                    // if client is tiled, set to floating
                    if self.clients.set_floating(&event.subwindow) {
                        self.arrange_clients();
                    }

                    let client = self.clients.get(&event.subwindow).unwrap();

                    let position = {
                        (
                            client.position.0 + client.size.0,
                            client.position.1 + client.size.1,
                        )
                    };

                    self.xlib.move_cursor(client.window, position);
                    self.xlib.grab_cursor();

                    self.resize_window = Some(MoveWindow {
                        key: event.subwindow,
                        cached_cursor_position: position,
                    });
                }
            }

            // reset on release
            xlib::ButtonRelease => {
                let event: &XButtonEvent = event.as_ref();

                if event.button == 3 && self.resize_window.is_some() {
                    self.resize_window = None;
                    self.xlib.release_cursor();
                }
            }

            xlib::MotionNotify => {
                let event = self.xlib.squash_event(xlib::MotionNotify);
                let event: &XMotionEvent = event.as_ref();

                if let Some(resize_window) = &mut self.resize_window {
                    info!("MotionNotify-resize");
                    let (x, y) = (
                        event.x - resize_window.cached_cursor_position.0,
                        event.y - resize_window.cached_cursor_position.1,
                    );

                    resize_window.cached_cursor_position = (event.x, event.y);

                    if let Some(client) =
                        self.clients.get_mut(&resize_window.key).into_option()
                    {
                        let size = &mut client.size;

                        size.0 = std::cmp::max(1, size.0 + x);
                        size.1 = std::cmp::max(1, size.1 + y);

                        self.xlib.resize_client(client);
                    }
                }
            }
            _ => {}
        }
    }

    fn rotate_virtual_screen_back(&mut self) {
        if let Some(dir) = self.last_rotation {
            self.rotate_virtual_screen(!dir);
        }
    }

    fn rotate_virtual_screen(&mut self, dir: Direction) {
        info!("rotateing VS: {:?}", dir);

        self.last_rotation = Some(dir);

        match dir {
            Direction::Left => self.clients.rotate_left(),
            Direction::Right => self.clients.rotate_right(),
        }

        self.arrange_clients();

        // focus first client in all visible clients
        let to_focus =
            self.clients.iter_visible().next().map(|(k, _)| k).cloned();

        if let Some(key) = to_focus {
            self.focus_client(&key);
        }
    }

    fn focus_master_stack(&mut self) {
        let k = self
            .clients
            .iter_master_stack()
            .map(|(k, _)| k)
            .next()
            .cloned();

        if let Some(k) = k {
            self.focus_client(&k);
        }
    }

    fn focus_aux_stack(&mut self) {
        let k = self
            .clients
            .iter_aux_stack()
            .map(|(k, _)| k)
            .next()
            .cloned();

        if let Some(k) = k {
            self.focus_client(&k);
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
        self.clients
            .arrange_virtual_screen(width, height, self.config.gap);

        self.clients
            .iter_visible()
            .for_each(|(_, c)| self.xlib.move_resize_client(c));

        self.hide_hidden_clients();

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
        let client = if let Some(transient_window) =
            self.xlib.get_transient_for_window(window)
        {
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

    fn button_notify(&mut self, event: &XEvent) {
        let event: &XButtonEvent = event.as_ref();

        self.focus_client(&event.subwindow);
        if let Some(client) = self.clients.get(&event.subwindow).into_option() {
            self.xlib.raise_client(client);
        }
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

impl Default for WMConfig {
    fn default() -> Self {
        Self {
            num_virtualscreens: 5,
            mod_key: Mod4Mask,
            gap: Some(2),
        }
    }
}

impl std::ops::Not for Direction {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        }
    }
}
