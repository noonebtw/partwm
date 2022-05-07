use std::{cell::RefCell, rc::Rc};

use log::{error, info};

use x11::xlib::{self, Window};

use crate::backends::structs::WindowType;
use crate::backends::window_event::{
    FullscreenEvent, FullscreenState, WindowNameEvent,
};
use crate::util::{Point, Size};
use crate::{
    backends::{
        keycodes::{MouseButton, VirtualKeyCode},
        window_event::{
            ButtonEvent, ConfigureEvent, KeyBind, KeyEvent, KeyState, MapEvent,
            ModifierKey, ModifierState, MotionEvent, MouseBind, WindowEvent,
        },
        xlib::XLib,
        WindowServerBackend,
    },
    clients::{Client, ClientEntry, ClientKey, ClientState},
};

use serde::Deserialize;

/**
Contains static config data for the window manager, the sort of stuff you might want to
be able to configure in a config file.
 */
#[derive(Debug, Deserialize)]
pub struct WMConfig {
    num_virtualscreens: usize,
    mod_key: ModifierKey,
    gap: Option<i32>,
    kill_clients_on_exit: bool,
    #[serde(default = "WMConfig::default_active_window_border_color")]
    active_window_border_color: String,
    #[serde(default = "WMConfig::default_inactive_window_border_color")]
    inactive_window_border_color: String,
    #[serde(default = "WMConfig::default_terminal")]
    terminal_command: (String, Vec<String>),
    border_width: Option<i32>,
}

impl WMConfig {
    fn default_active_window_border_color() -> String {
        "#ffffff".to_string()
    }

    fn default_inactive_window_border_color() -> String {
        "#444444".to_string()
    }

    fn default_terminal() -> (String, Vec<String>) {
        ("xterm".to_string(), vec![])
    }
}

impl Default for WMConfig {
    fn default() -> Self {
        Self {
            num_virtualscreens: 10,
            mod_key: ModifierKey::Super,
            gap: Some(2),
            kill_clients_on_exit: false,
            active_window_border_color:
                Self::default_active_window_border_color(),
            inactive_window_border_color:
                Self::default_inactive_window_border_color(),
            terminal_command: Self::default_terminal(),
            border_width: Some(1),
        }
    }
}

pub struct WindowManager<B = XLib>
where
    B: WindowServerBackend,
{
    clients: ClientState,
    move_resize_window: MoveResizeInfo,
    keybinds: Rc<RefCell<Vec<KeyBinding<B>>>>,
    backend: B,

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

#[derive(Debug)]
struct MoveInfoInner {
    window: Window,
    starting_cursor_pos: Point<i32>,
    starting_window_pos: Point<i32>,
}

#[derive(Debug)]
struct ResizeInfoInner {
    window: Window,
    starting_cursor_pos: Point<i32>,
    starting_window_size: Size<i32>,
}

use derivative::*;

#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
struct KeyBinding<B: WindowServerBackend> {
    key: KeyBind,
    closure: Rc<dyn Fn(&mut WindowManager<B>, &KeyEvent<B::Window>)>,
}

impl<B: WindowServerBackend> KeyBinding<B> {
    pub fn new<F>(key: KeyBind, cb: F) -> Self
    where
        F: Fn(&mut WindowManager<B>, &KeyEvent<B::Window>),
        F: 'static,
    {
        Self {
            key,
            closure: Rc::new(cb),
        }
    }

    pub fn call(&self, wm: &mut WindowManager<B>, ev: &KeyEvent<B::Window>) {
        (self.closure)(wm, ev);
    }
}

impl<B> WindowManager<B>
where
    B: WindowServerBackend<Window = xlib::Window>,
{
    pub fn new(config: WMConfig) -> Self {
        let backend = B::build();

        let clients = ClientState::new()
            .with_virtualscreens(config.num_virtualscreens)
            .with_gap(config.gap.unwrap_or(1))
            .with_border(config.border_width.unwrap_or(1))
            .with_screen_size(backend.screen_size());

        Self {
            clients,
            move_resize_window: MoveResizeInfo::None,
            keybinds: Rc::new(RefCell::new(Vec::new())),
            backend,
            config,
        }
        .init()
    }

    fn init(mut self) -> Self {
        self.backend.add_keybind(
            MouseBind::new(MouseButton::Left)
                .with_mod(self.config.mod_key)
                .into(),
        );
        self.backend.add_keybind(
            MouseBind::new(MouseButton::Middle)
                .with_mod(self.config.mod_key)
                .into(),
        );
        self.backend.add_keybind(
            MouseBind::new(MouseButton::Right)
                .with_mod(self.config.mod_key)
                .into(),
        );

        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::P).with_mod(self.config.mod_key),
            |wm, _| {
                wm.spawn(
                    &"dmenu_run",
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

        // self.add_keybind(KeyBinding::new(
        //     KeyBind::new(VirtualKeyCode::Print),
        //     |wm, _| wm.spawn("screenshot.sh", &[]),
        // ));

        // self.add_keybind(KeyBinding::new(
        //     KeyBind::new(VirtualKeyCode::Print).with_mod(ModifierKey::Shift),
        //     |wm, _| wm.spawn("screenshot.sh", &["-edit"]),
        // ));

        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::M).with_mod(self.config.mod_key),
            |wm, _| wm.handle_switch_stack(),
        ));

        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::F).with_mod(self.config.mod_key),
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
            KeyBind::new(VirtualKeyCode::Q).with_mod(self.config.mod_key),
            |wm, _| wm.kill_client(),
        ));

        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::Q)
                .with_mod(self.config.mod_key)
                .with_mod(ModifierKey::Shift),
            |wm, _| wm.quit(),
        ));

        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::Return)
                .with_mod(self.config.mod_key)
                .with_mod(ModifierKey::Shift),
            |wm, _| {
                wm.spawn(
                    &wm.config.terminal_command.0,
                    &wm.config.terminal_command.1,
                )
            },
        ));

        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::J).with_mod(self.config.mod_key),
            |wm, _| wm.move_focus(Direction::south()),
        ));

        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::K).with_mod(self.config.mod_key),
            |wm, _| wm.move_focus(Direction::north()),
        ));

        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::H).with_mod(self.config.mod_key),
            |wm, _| wm.move_focus(Direction::west()),
        ));

        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::L).with_mod(self.config.mod_key),
            |wm, _| wm.move_focus(Direction::east()),
        ));

        // resize master stack

        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::K)
                .with_mod(self.config.mod_key)
                .with_mod(ModifierKey::Shift),
            |wm, _| {
                wm.clients.change_master_size(0.1);
                wm.arrange_clients();
            },
        ));

        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::J)
                .with_mod(self.config.mod_key)
                .with_mod(ModifierKey::Shift),
            |wm, _| {
                wm.clients.change_master_size(-0.1);
                wm.arrange_clients();
            },
        ));

        self.add_vs_switch_keybinds();

        self.backend.set_active_window_border_color(
            &self.config.active_window_border_color,
        );
        self.backend.set_inactive_window_border_color(
            &self.config.inactive_window_border_color,
        );

        // add all already existing windows to the WM
        if let Some(windows) = self.backend.all_windows() {
            windows
                .into_iter()
                .for_each(|window| self.new_client(window));
        }

        self
    }

    fn add_keybind(&mut self, keybind: KeyBinding<B>) {
        self.backend.add_keybind((&keybind.key).into());
        self.keybinds.borrow_mut().push(keybind);
    }

    fn add_vs_switch_keybinds(&mut self) {
        // Old keybinds

        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::Left).with_mod(self.config.mod_key),
            |wm, _| wm.rotate_virtual_screen(Direction::West(1)),
        ));

        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::H)
                .with_mod(self.config.mod_key)
                .with_mod(ModifierKey::Shift),
            |wm, _| wm.rotate_virtual_screen(Direction::West(1)),
        ));

        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::Right).with_mod(self.config.mod_key),
            |wm, _| wm.rotate_virtual_screen(Direction::East(1)),
        ));

        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::L)
                .with_mod(self.config.mod_key)
                .with_mod(ModifierKey::Shift),
            |wm, _| wm.rotate_virtual_screen(Direction::East(1)),
        ));

        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::Tab).with_mod(self.config.mod_key),
            |wm, _| wm.rotate_virtual_screen_back(),
        ));

        // Mod + Num

        // Press Mod + `1` to move go to the `1`th virtual screen
        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::One).with_mod(self.config.mod_key),
            |wm, _| wm.go_to_nth_virtual_screen(1),
        ));

        // Press Mod + `2` to move go to the `2`th virtual screen
        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::Two).with_mod(self.config.mod_key),
            |wm, _| wm.go_to_nth_virtual_screen(2),
        ));

        // Press Mod + `3` to move go to the `3`th virtual screen
        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::Three).with_mod(self.config.mod_key),
            |wm, _| wm.go_to_nth_virtual_screen(3),
        ));

        // Press Mod + `4` to move go to the `4`th virtual screen
        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::Four).with_mod(self.config.mod_key),
            |wm, _| wm.go_to_nth_virtual_screen(4),
        ));

        // Press Mod + `5` to move go to the `5`th virtual screen
        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::Five).with_mod(self.config.mod_key),
            |wm, _| wm.go_to_nth_virtual_screen(5),
        ));

        // Press Mod + `6` to move go to the `6`th virtual screen
        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::Six).with_mod(self.config.mod_key),
            |wm, _| wm.go_to_nth_virtual_screen(6),
        ));

        // Press Mod + `7` to move go to the `7`th virtual screen
        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::Seven).with_mod(self.config.mod_key),
            |wm, _| wm.go_to_nth_virtual_screen(7),
        ));

        // Press Mod + `8` to move go to the `8`th virtual screen
        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::Eight).with_mod(self.config.mod_key),
            |wm, _| wm.go_to_nth_virtual_screen(8),
        ));

        // Press Mod + `9` to move go to the `9`th virtual screen
        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::Nine).with_mod(self.config.mod_key),
            |wm, _| wm.go_to_nth_virtual_screen(9),
        ));

        // Press Mod + `0` to move go to the `0`th virtual screen
        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::Zero).with_mod(self.config.mod_key),
            |wm, _| wm.go_to_nth_virtual_screen(10),
        ));
    }

    #[allow(unused_mut)]
    pub fn run(mut self) -> ! {
        loop {
            let event = self.backend.next_event();

            match event {
                WindowEvent::KeyEvent(event) => {
                    if event.state == KeyState::Pressed {
                        self.handle_keybinds(&event);
                    }
                }
                WindowEvent::ButtonEvent(event) => {
                    self.button_event(&event);
                }
                WindowEvent::MapRequestEvent(MapEvent { window }) => {
                    self.backend.handle_event(event);

                    if !self.clients.contains(&window) {
                        self.new_client(window);
                    }
                }
                WindowEvent::UnmapEvent(event) => {
                    self.clients.remove(&event.window);
                    self.arrange_clients();
                }
                WindowEvent::EnterEvent(event) => {
                    self.focus_client(&event.window, false);
                }
                WindowEvent::MotionEvent(event) => {
                    self.do_move_resize_window(&event);
                }
                WindowEvent::ConfigureEvent(ConfigureEvent {
                    window, ..
                }) => {
                    match self.clients.get(&window) {
                        ClientEntry::Tiled(client)
                        | ClientEntry::Floating(client) => {
                            self.backend.configure_window(
                                window,
                                Some(client.size),
                                Some(client.position),
                                None,
                            )
                        }
                        ClientEntry::Vacant => self.backend.handle_event(event),
                    }
                    // TODO
                    // match self.clients.get(&event.window).into_option() {
                    //     Some(client) => self
                    //         .xlib
                    //         .configure_client(client, self.clients.get_border()),
                    //     None => self.xlib.configure_window(event),
                    // }
                }
                WindowEvent::FullscreenEvent(FullscreenEvent {
                    window,
                    state,
                }) => {
                    if match state {
                        FullscreenState::On => {
                            self.clients.set_fullscreen(&window, true)
                        }
                        FullscreenState::Off => {
                            self.clients.set_fullscreen(&window, false)
                        }
                        FullscreenState::Toggle => {
                            self.clients.toggle_fullscreen(&window)
                        }
                    } {
                        if let Some(client) =
                            self.clients.get(&window).into_option()
                        {
                            self.backend.configure_window(
                                window,
                                None,
                                None,
                                if client.is_fullscreen() {
                                    Some(0)
                                } else {
                                    Some(self.clients.get_border())
                                },
                            );
                        };

                        self.arrange_clients();
                    }
                }
                WindowEvent::WindowNameEvent(WindowNameEvent { .. }) => {
                    info!("{:#?}", event);
                }

                // i dont think i actually have to handle destroy notify events.
                // every window should be unmapped regardless
                // xlib::DestroyNotify => self.destroy_notify(&event),
                _ => {}
            }
        }
    }

    fn quit(&self) -> ! {
        // TODO: should the window manager kill all clients on exit? probably
        if self.config.kill_clients_on_exit {
            self.clients
                .iter_all_clients()
                .for_each(|(&window, _)| self.backend.kill_window(window));
        }

        info!("Goodbye.");

        std::process::exit(0);
    }

    fn kill_client(&mut self) {
        if let Some(client) = self.clients.get_focused().into_option() {
            self.backend.kill_window(client.window);
        }
    }

    // TODO: change this somehow cuz I'm not a big fan of this "hardcoded" keybind stuff
    fn handle_keybinds(&mut self, event: &KeyEvent<B::Window>) {
        // I'm not sure if this has to be a Rc<RefCell>> or if it would be better as a Cell<>
        let keybinds = self.keybinds.clone();

        for kb in keybinds.borrow().iter() {
            if kb.key.key == event.keycode
                && kb.key.modifiers == event.modifierstate
            {
                kb.call(self, event);
            }
        }
    }

    fn handle_switch_stack(&mut self) {
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
        info!("rotating VS: {:?}", dir);

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
            .for_each(|(_, c)| self.backend.hide_window(c.window));
    }

    fn raise_floating_clients(&self) {
        self.clients
            .iter_floating()
            .for_each(|(_, c)| self.backend.raise_window(c.window));

        self.clients
            .iter_transient()
            .for_each(|(_, c)| self.backend.raise_window(c.window));

        //raise fullscreen windows
        self.clients
            .iter_current_screen()
            .filter(|(_, c)| c.is_fullscreen())
            .for_each(|(_, c)| self.backend.raise_window(c.window));
    }

    fn arrange_clients(&mut self) {
        self.clients.iter_visible().for_each(|(_, c)| {
            self.backend.move_window(c.window, c.position);
            self.backend.resize_window(c.window, c.size);
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
            self.backend.unfocus_window(old.window);
        }

        match new {
            ClientEntry::Floating(new) => {
                self.backend.focus_window(new.window);

                if try_raise {
                    self.backend.raise_window(new.window);
                }
            }
            ClientEntry::Tiled(new) => {
                self.backend.focus_window(new.window);
            }
            _ => {}
        }
    }

    fn new_client(&mut self, window: Window) {
        let client = match self.backend.get_window_type(window) {
            WindowType::Normal => Client::new_default(window),
            window_type @ _ => Client::new_default(window)
                .with_window_type(window_type)
                .with_size(
                    self.backend
                        .get_window_size(window)
                        .unwrap_or((100, 100).into()),
                )
                .with_parent_window(self.backend.get_parent_window(window)),
        };

        self.backend.configure_window(
            window,
            None,
            None,
            Some(self.clients.get_border()),
        );

        info!("new client: {:#?}", client);

        self.clients.insert(client).unwrap();
        self.arrange_clients();

        self.focus_client(&window, true);
    }

    /// ensure event.subwindow refers to a valid client.
    fn start_move_resize_window(&mut self, event: &ButtonEvent<B::Window>) {
        let window = event.window; // xev.subwindow

        if !self.clients.get(&window).is_fullscreen() {
            match event.keycode {
                MouseButton::Left => {
                    if self.clients.set_floating(&window) {
                        self.arrange_clients();
                    }

                    self.move_resize_window =
                        MoveResizeInfo::Move(MoveInfoInner {
                            window,
                            starting_cursor_pos: event.cursor_position,
                            starting_window_pos: self
                                .clients
                                .get(&window)
                                .unwrap()
                                .position,
                        });
                }
                MouseButton::Right => {
                    if self.clients.set_floating(&window) {
                        self.arrange_clients();
                    }

                    let client = self.clients.get(&window).unwrap();

                    let corner_pos = client.position + client.size.into();

                    self.backend.move_cursor(None, corner_pos.into());
                    self.backend.grab_cursor();

                    self.move_resize_window =
                        MoveResizeInfo::Resize(ResizeInfoInner {
                            window,
                            starting_cursor_pos: corner_pos.into(),
                            starting_window_size: client.size,
                        });
                }
                _ => {}
            }
        }
    }

    fn end_move_resize_window(&mut self, event: &ButtonEvent<B::Window>) {
        match event.keycode {
            MouseButton::Left => {
                self.move_resize_window = MoveResizeInfo::None;
            }
            MouseButton::Right => {
                self.move_resize_window = MoveResizeInfo::None;
                self.backend.ungrab_cursor();
            }
            _ => {}
        }
    }

    fn do_move_resize_window(&mut self, event: &MotionEvent<B::Window>) {
        match &self.move_resize_window {
            MoveResizeInfo::Move(info) => {
                let (x, y) = (
                    event.position.x - info.starting_cursor_pos.x,
                    event.position.y - info.starting_cursor_pos.y,
                );

                if let Some(client) =
                    self.clients.get_mut(&info.window).into_option()
                {
                    let position = &mut client.position;

                    position.x = info.starting_window_pos.x + x;
                    position.y = info.starting_window_pos.y + y;

                    self.backend.move_window(client.window, client.position);
                }
            }
            MoveResizeInfo::Resize(info) => {
                let (x, y) = (
                    event.position.x - info.starting_cursor_pos.x,
                    event.position.y - info.starting_cursor_pos.y,
                );

                if let Some(client) =
                    self.clients.get_mut(&info.window).into_option()
                {
                    let size = &mut client.size;

                    size.width =
                        std::cmp::max(1, info.starting_window_size.width + x);
                    size.height =
                        std::cmp::max(1, info.starting_window_size.height + y);

                    self.backend.resize_window(client.window, client.size);
                }
            }
            _ => {}
        }
    }

    fn button_event(&mut self, event: &ButtonEvent<B::Window>) {
        match event.state {
            KeyState::Pressed => {
                self.focus_client(&event.window, true);

                match event.keycode {
                    MouseButton::Left | MouseButton::Right => {
                        match self.move_resize_window {
                            MoveResizeInfo::None
                                if ModifierState::from([self
                                    .config
                                    .mod_key])
                                .eq(&event.modifierstate)
                                    && self.clients.contains(&event.window) =>
                            {
                                self.start_move_resize_window(event)
                            }
                            _ => {}
                        }
                    }
                    MouseButton::Middle => {
                        self.clients.toggle_floating(&event.window);
                        self.arrange_clients();
                    }
                    _ => {}
                }
            }
            KeyState::Released => match self.move_resize_window {
                MoveResizeInfo::None => {}
                _ => {
                    self.end_move_resize_window(event);
                }
            },
        }
    }

    pub fn spawn<'a, S, I>(&self, command: S, args: I)
    where
        S: AsRef<str> + AsRef<std::ffi::OsStr>,
        I: IntoIterator<Item = S> + std::fmt::Debug,
    {
        info!("spawn: {:?} {:?}", AsRef::<str>::as_ref(&command), args);
        match std::process::Command::new(AsRef::<std::ffi::OsStr>::as_ref(
            &command,
        ))
        .args(args)
        .spawn()
        {
            Ok(_) => {}
            Err(err) => {
                error!(
                    "Failed to spawn {:?}: {:?}",
                    AsRef::<str>::as_ref(&command),
                    err
                );
            }
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
