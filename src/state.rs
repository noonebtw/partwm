use std::rc::Rc;

use log::{error, info};

use x11::xlib::{
    self, Window, XButtonPressedEvent, XButtonReleasedEvent, XEvent, XKeyEvent,
    XMotionEvent,
};
use xlib::{
    XConfigureRequestEvent, XCrossingEvent, XDestroyWindowEvent,
    XMapRequestEvent, XUnmapEvent,
};

use crate::{
    backends::{
        keycodes::{MouseButton, VirtualKeyCode},
        window_event::{
            ButtonEvent, KeyBind, KeyEvent, ModifierKey, ModifierState,
        },
        xlib::XLib,
        WindowServerBackend,
    },
    clients::{Client, ClientEntry, ClientKey, ClientState},
};

/**
Contains static config data for the window manager, the sort of stuff you might want to
be able to configure in a config file.
*/
pub struct WMConfig {
    num_virtualscreens: usize,
    mod_key: ModifierKey,
    gap: Option<i32>,
}

pub struct WindowManager<B = XLib>
where
    B: WindowServerBackend,
{
    clients: ClientState,
    move_resize_window: MoveResizeInfo,
    keybinds: Vec<KeyBinding<B>>,
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
        let backend = B::new();

        let clients = ClientState::new()
            .with_virtualscreens(config.num_virtualscreens)
            .with_gap(config.gap.unwrap_or(1))
            .with_border(1)
            .with_screen_size(backend.screen_size());

        Self {
            clients,
            move_resize_window: MoveResizeInfo::None,
            keybinds: Vec::new(),
            backend,
            config,
        }
        .init()
    }

    fn init(mut self) -> Self {
        // TODO: fix keybinds for moving windows and stuff
        // self.xlib.add_global_keybind(KeyOrButton::button(
        //     1,
        //     self.config.mod_key,
        //     ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
        // ));
        // self.xlib.add_global_keybind(KeyOrButton::button(
        //     2,
        //     self.config.mod_key,
        //     ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
        // ));
        // self.xlib.add_global_keybind(KeyOrButton::button(
        //     3,
        //     self.config.mod_key,
        //     ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
        // ));

        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::P).with_mod(self.config.mod_key),
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
            KeyBind::new(VirtualKeyCode::Snapshot),
            |wm, _| wm.spawn("screenshot.sh", &[]),
        ));

        self.add_keybind(KeyBinding::new(
            KeyBind::new(VirtualKeyCode::Snapshot).with_mod(ModifierKey::Shift),
            |wm, _| wm.spawn("screenshot.sh", &["-edit"]),
        ));

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
            |wm, _| wm.spawn("alacritty", &[]),
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

        //self.xlib.init();

        self
    }

    fn add_keybind(&mut self, keybind: KeyBinding<B>) {
        //self.xlib.add_global_keybind(keybind.key);
        self.keybinds.push(keybind);
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
                // xlib::MapRequest => self.map_request(&event),
                // xlib::UnmapNotify => self.unmap_notify(&event),
                // xlib::ConfigureRequest => self.configure_request(&event),
                // xlib::EnterNotify => self.enter_notify(&event),
                // xlib::DestroyNotify => self.destroy_notify(&event),
                // xlib::ButtonPress => self.button_press(event.as_ref()),
                // xlib::ButtonRelease => self.button_release(event.as_ref()),
                // xlib::MotionNotify => self.motion_notify(event.as_ref()),
                // xlib::KeyPress => self.handle_keybinds(event.as_ref()),
                _ => {}
            }
        }
    }

    fn quit(&self) -> ! {
        info!("Goodbye.");

        std::process::exit(0);
    }

    fn kill_client(&mut self) {
        if let Some(client) = self.clients.get_focused().into_option() {
            self.backend.kill_window(client.window);
        }
    }

    // TODO: change this somehow cuz I'm not a big fan of this "hardcoded" keybind stuff
    fn handle_keybinds(&mut self, event: &XKeyEvent) {
        //let clean_mask = self.xlib.get_clean_mask();
        // TODO: Fix this
        // for kb in self.keybinds.clone().into_iter() {
        //     if let KeyOrButton::Key(keycode, modmask) = kb.key {
        //         if keycode as u32 == event.keycode
        //             && modmask & clean_mask == event.state & clean_mask
        //         {
        //             (kb.closure)(self, event);
        //         }
        //     }
        // }
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
            .for_each(|(_, c)| self.backend.hide_window(c.window));
    }

    fn raise_floating_clients(&self) {
        self.clients
            .iter_floating()
            .for_each(|(_, c)| self.backend.raise_window(c.window));

        self.clients
            .iter_transient()
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
        info!("new client: {:?}", window);
        let client = if let Some(transient_window) =
            self.backend.get_parent_window(window)
        {
            Client::new_transient(
                window,
                self.backend.get_window_size(window).unwrap_or((100, 100)),
                transient_window,
            )
        } else {
            Client::new_default(window)
        };

        //self.xlib
        //.configure_client(&client, self.clients.get_border());
        self.clients.insert(client).unwrap();
        self.arrange_clients();

        //self.xlib.map_window(window);

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

        // match self.clients.get(&event.window).into_option() {
        //     Some(client) => self
        //         .xlib
        //         .configure_client(client, self.clients.get_border()),
        //     None => self.xlib.configure_window(event),
        // }
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

                // TODO fix backend cursor api
                //self.xlib.move_cursor(None, corner_pos);
                //self.xlib.grab_cursor();

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
            // TODO fix backend cursor api
            //self.xlib.release_cursor();
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

                    self.backend.move_window(client.window, client.position);
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

                    self.backend.resize_window(client.window, client.size);
                }
            }
            _ => {}
        }
    }

    fn button_press(&mut self, event: &ButtonEvent<B::Window>) {
        self.focus_client(&event.window, true);

        match event.keycode {
            MouseButton::Left | MouseButton::Right => {
                match self.move_resize_window {
                    MoveResizeInfo::None
                        if ModifierState::from([self.config.mod_key])
                            .eq_ignore_lock(&event.modifierstate)
                            && self.clients.contains(&event.window) =>
                    {
                        //self.start_move_resize_window(event)
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

impl Default for WMConfig {
    fn default() -> Self {
        Self {
            num_virtualscreens: 10,
            mod_key: ModifierKey::Super,
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
