use std::rc::Rc;

use log::{error, info};

use x11::xlib::{
    self, Mod4Mask, ShiftMask, Window, XButtonPressedEvent,
    XButtonReleasedEvent, XEvent, XKeyEvent, XMotionEvent,
};
use xlib::{
    ButtonPressMask, ButtonReleaseMask, PointerMotionMask,
    XConfigureRequestEvent, XCrossingEvent, XDestroyWindowEvent,
    XMapRequestEvent, XUnmapEvent,
};

use crate::{
    clients::{Client, ClientEntry, ClientKey, ClientState},
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
    move_resize_window: MoveResizeInfo,
    keybinds: Vec<KeyBinding>,
    xlib: XLib,

    config: WMConfig,
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
    West(usize),
    East(usize),
    North(usize),
    South(usize),
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
    pub fn new(config: WMConfig) -> Self {
        let xlib = XLib::new();

        let clients = ClientState::new()
            .with_virtualscreens(config.num_virtualscreens)
            .with_gap(config.gap.unwrap_or(1))
            .with_border(1)
            .with_screen_size(xlib.dimensions());

        Self {
            clients,
            move_resize_window: MoveResizeInfo::None,
            keybinds: Vec::new(),
            xlib,
            config,
        }
        .init()
    }

    fn init(mut self) -> Self {
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
            self.xlib.make_key("Print", 0),
            |wm, _| wm.spawn("screenshot.sh", &[]),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("Print", ShiftMask),
            |wm, _| wm.spawn("screenshot.sh", &["-edit"]),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("M", self.config.mod_key),
            Self::handle_switch_stack,
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("F", self.config.mod_key),
            |wm, _| {
                wm.clients
                    .get_focused()
                    .into_option()
                    .map(|c| c.key())
                    .and_then(|k| Some(wm.clients.toggle_floating(&k)));

                wm.arrange_clients();
            },
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
            self.xlib
                .make_key("Return", self.config.mod_key | ShiftMask),
            |wm, _| wm.spawn("alacritty", &[]),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("J", self.config.mod_key),
            |wm, _| wm.move_focus(Direction::south()),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("K", self.config.mod_key),
            |wm, _| wm.move_focus(Direction::north()),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("H", self.config.mod_key),
            |wm, _| wm.move_focus(Direction::west()),
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("L", self.config.mod_key),
            |wm, _| wm.move_focus(Direction::east()),
        ));

        // resize master stack

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("K", self.config.mod_key | ShiftMask),
            |wm, _| {
                wm.clients.change_master_size(0.1);
                wm.arrange_clients();
            },
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("J", self.config.mod_key | ShiftMask),
            |wm, _| {
                wm.clients.change_master_size(-0.1);
                wm.arrange_clients();
            },
        ));

        self.add_vs_switch_keybinds();

        self.xlib.init();

        self
    }

    fn add_keybind(&mut self, keybind: KeyBinding) {
        self.xlib.add_global_keybind(keybind.key);
        self.keybinds.push(keybind);
    }

    fn add_vs_switch_keybinds(&mut self) {
        fn rotate_west<const N: usize>(wm: &mut WindowManager, _: &XKeyEvent) {
            wm.rotate_virtual_screen(Direction::West(N));
        }

        fn rotate_east<const N: usize>(wm: &mut WindowManager, _: &XKeyEvent) {
            wm.rotate_virtual_screen(Direction::East(N));
        }

        fn goto_nth<const N: usize>(wm: &mut WindowManager, _: &XKeyEvent) {
            wm.go_to_nth_virtual_screen(N)
        }

        // Old keybinds

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("Left", self.config.mod_key),
            rotate_west::<1>,
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("H", self.config.mod_key | ShiftMask),
            rotate_west::<1>,
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("Right", self.config.mod_key),
            rotate_east::<1>,
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("L", self.config.mod_key | ShiftMask),
            rotate_east::<1>,
        ));

        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("Tab", self.config.mod_key),
            |wm, _| wm.rotate_virtual_screen_back(),
        ));

        // Mod + Num

        // Press Mod + `1` to move go to the `1`th virtual screen
        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("1", self.config.mod_key),
            goto_nth::<1>,
        ));

        // Press Mod + `2` to move go to the `2`th virtual screen
        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("2", self.config.mod_key),
            goto_nth::<2>,
        ));

        // Press Mod + `3` to move go to the `3`th virtual screen
        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("3", self.config.mod_key),
            goto_nth::<3>,
        ));

        // Press Mod + `4` to move go to the `4`th virtual screen
        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("4", self.config.mod_key),
            goto_nth::<4>,
        ));

        // Press Mod + `5` to move go to the `5`th virtual screen
        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("5", self.config.mod_key),
            goto_nth::<5>,
        ));

        // Press Mod + `6` to move go to the `6`th virtual screen
        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("6", self.config.mod_key),
            goto_nth::<6>,
        ));

        // Press Mod + `7` to move go to the `7`th virtual screen
        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("7", self.config.mod_key),
            goto_nth::<7>,
        ));

        // Press Mod + `8` to move go to the `8`th virtual screen
        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("8", self.config.mod_key),
            goto_nth::<8>,
        ));

        // Press Mod + `9` to move go to the `9`th virtual screen
        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("9", self.config.mod_key),
            goto_nth::<9>,
        ));

        // Press Mod + `0` to move go to the `0`th virtual screen
        self.add_keybind(KeyBinding::new(
            self.xlib.make_key("0", self.config.mod_key),
            goto_nth::<10>,
        ));
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

    fn kill_client(&mut self, _event: &XKeyEvent) {
        if let Some(client) = self.clients.get_focused().into_option() {
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

    fn handle_switch_stack(&mut self, _event: &XKeyEvent) {
        if let Some(client) =
            self.clients.get_focused().into_option().map(|c| c.key())
        {
            info!("Switching stack for window {:?}", client);
            self.clients.switch_stack_for_client(&client);
        }

        self.arrange_clients();
    }

    fn rotate_virtual_screen_back(&mut self) {
        self.clients.rotate_back();

        self.arrange_clients();
    }

    fn go_to_nth_virtual_screen(&mut self, n: usize) {
        self.clients.go_to_nth_virtualscreen(n - 1);
        self.arrange_clients();
    }

    fn rotate_virtual_screen(&mut self, dir: Direction) {
        info!("rotateing VS: {:?}", dir);

        match dir {
            Direction::West(n) => self.clients.rotate_left(n),
            Direction::East(n) => self.clients.rotate_right(n),
            _ => {}
        }

        self.arrange_clients();
    }

    fn focus_any(&mut self) {
        // focus first client in all visible clients
        let to_focus =
            self.clients.iter_visible().next().map(|(k, _)| k).cloned();

        if let Some(key) = to_focus {
            self.focus_client(&key, false);
        }
    }

    fn focus_master_stack(&mut self) {
        let focused = self.clients.get_focused().into_option().map(|c| c.key());

        let k = self
            .clients
            .iter_floating_visible()
            .chain(self.clients.iter_master_stack())
            .map(|(k, _)| k)
            // get the first client on the stack thats not already focused
            .filter(|&&k| focused.map(|f| f != k).unwrap_or(true))
            .next()
            .cloned();

        if let Some(k) = k {
            self.focus_client(&k, false);
        }
    }

    fn focus_aux_stack(&mut self) {
        let focused = self.clients.get_focused().into_option().map(|c| c.key());

        let k = self
            .clients
            .iter_floating_visible()
            .chain(self.clients.iter_aux_stack())
            .map(|(k, _)| k)
            // get the first client on the stack thats not already focused
            .filter(|&&k| focused.map(|f| f != k).unwrap_or(true))
            .next()
            .cloned();

        if let Some(k) = k {
            self.focus_client(&k, false);
        }
    }

    fn focus_up(&mut self) {
        let focused = self.clients.get_focused().into_option().map(|c| c.key());

        let k = focused.and_then(|focused| {
            self.clients
                .get_stack_for_client(&focused)
                .and_then(|stack| {
                    stack
                        .iter()
                        .rev()
                        .skip_while(|&&k| k != focused)
                        .skip(1)
                        .next()
                        .cloned()
                })
        });

        if let Some(k) = k {
            self.focus_client(&k, false);
        }
    }

    fn focus_down(&mut self) {
        let focused = self.clients.get_focused().into_option().map(|c| c.key());

        let k = focused.and_then(|focused| {
            self.clients
                .get_stack_for_client(&focused)
                .and_then(|stack| {
                    stack
                        .iter()
                        .skip_while(|&&k| k != focused)
                        .skip(1)
                        .next()
                        .cloned()
                })
        });

        if let Some(k) = k {
            self.focus_client(&k, false);
        }
    }

    fn move_focus(&mut self, dir: Direction) {
        match dir {
            Direction::East(_) => self.focus_aux_stack(),
            Direction::West(_) => self.focus_master_stack(),
            Direction::North(_) => self.focus_up(),
            Direction::South(_) => self.focus_down(),
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
        self.clients.iter_visible().for_each(|(_, c)| {
            self.xlib.move_resize_client(c);
            //self.xlib.expose_client(c);
        });

        self.hide_hidden_clients();

        self.raise_floating_clients();

        // if no visible client is focused, focus any.
        if !self
            .clients
            .iter_visible()
            .any(|(k, _)| self.clients.is_focused(k))
        {
            self.focus_any();
        }
    }

    fn focus_client<K>(&mut self, key: &K, try_raise: bool)
    where
        K: ClientKey,
    {
        let (new, old) = self.clients.focus_client(key);

        if let Some(old) = old.into_option() {
            self.xlib.unfocus_client(old);
        }

        match new {
            ClientEntry::Floating(new) => {
                self.xlib.focus_client(new);

                if try_raise {
                    self.xlib.raise_client(new);
                }
            }
            ClientEntry::Tiled(new) => {
                self.xlib.focus_client(new);
            }
            _ => {}
        }
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

        self.xlib
            .configure_client(&client, self.clients.get_border());
        self.clients.insert(client).unwrap();
        self.arrange_clients();

        self.xlib.map_window(window);

        self.focus_client(&window, true);
    }

    fn map_request(&mut self, event: &XEvent) {
        let event: &XMapRequestEvent = event.as_ref();

        if !self.clients.contains(&event.window) {
            self.new_client(event.window);
        }
    }

    fn unmap_notify(&mut self, event: &XEvent) {
        let event: &XUnmapEvent = event.as_ref();

        self.clients.remove(&event.window);

        self.arrange_clients();
    }

    fn destroy_notify(&mut self, event: &XEvent) {
        let event: &XDestroyWindowEvent = event.as_ref();

        self.clients.remove(&event.window);

        self.arrange_clients();
    }

    fn configure_request(&mut self, event: &XEvent) {
        let event: &XConfigureRequestEvent = event.as_ref();

        match self.clients.get(&event.window).into_option() {
            Some(client) => self
                .xlib
                .configure_client(client, self.clients.get_border()),
            None => self.xlib.configure_window(event),
        }
    }

    fn enter_notify(&mut self, event: &XEvent) {
        let event: &XCrossingEvent = event.as_ref();

        self.focus_client(&event.window, false);
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
                    starting_window_pos: self
                        .clients
                        .get(&window)
                        .unwrap()
                        .position,
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

                self.move_resize_window =
                    MoveResizeInfo::Resize(ResizeInfoInner {
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

                if let Some(client) =
                    self.clients.get_mut(&info.window).into_option()
                {
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

                if let Some(client) =
                    self.clients.get_mut(&info.window).into_option()
                {
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
        self.focus_client(&event.subwindow, true);

        match event.button {
            1 | 3 => match self.move_resize_window {
                MoveResizeInfo::None
                    if self
                        .xlib
                        .are_masks_equal(event.state, self.config.mod_key)
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

impl Default for WMConfig {
    fn default() -> Self {
        Self {
            num_virtualscreens: 10,
            mod_key: Mod4Mask,
            gap: Some(2),
        }
    }
}

impl Direction {
    fn west() -> Self {
        Direction::West(1)
    }

    fn east() -> Self {
        Direction::East(1)
    }

    fn north() -> Self {
        Direction::North(1)
    }

    fn south() -> Self {
        Direction::South(1)
    }
}

impl std::ops::Not for Direction {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Direction::West(n) => Direction::East(n),
            Direction::East(n) => Direction::West(n),
            Direction::North(n) => Direction::North(n),
            Direction::South(n) => Direction::South(n),
        }
    }
}
