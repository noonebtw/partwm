#![allow(dead_code)]
//use x11::xlib::Window;

use super::keycodes::{MouseButton, VirtualKeyCode};

#[derive(Debug)]
pub enum WindowEvent<Window> {
    KeyEvent(KeyEvent<Window>),
    ButtonEvent(ButtonEvent<Window>),
    MotionEvent(MotionEvent<Window>),
    MapRequestEvent(MapEvent<Window>),
    MapEvent(MapEvent<Window>),
    UnmapEvent(UnmapEvent<Window>),
    CreateEvent(CreateEvent<Window>),
    DestroyEvent(DestroyEvent<Window>),
    EnterEvent(EnterEvent<Window>),
    ConfigureEvent(ConfigureEvent<Window>),
    FullscreenEvent(FullscreenEvent<Window>), //1 { window: Window, event: 1 },
}

#[derive(Debug)]
pub enum KeyState {
    Pressed,
    Released,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
#[repr(u8)]
pub enum ModifierKey {
    Shift,
    ShiftLock,
    Control,
    Alt,
    AltGr,
    /// Windows key on most keyboards
    Super,
    NumLock,
}

#[derive(Default, Debug, Clone)]
pub struct ModifierState {
    modifiers: std::collections::HashSet<ModifierKey>,
}

impl ModifierState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_modifier(&mut self, modifier: ModifierKey) {
        self.modifiers.insert(modifier);
    }

    pub fn unset_modifier(&mut self, modifier: ModifierKey) {
        self.modifiers.remove(&modifier);
    }

    pub fn get_modifier(&mut self, modifier: ModifierKey) -> bool {
        self.modifiers.contains(&modifier)
    }
}

#[derive(Debug)]
pub struct KeyEvent<Window> {
    pub window: Window,
    pub state: KeyState,
    pub keycode: VirtualKeyCode,
    pub modifierstate: ModifierState,
}

impl<Window> KeyEvent<Window> {
    pub fn new(
        window: Window,
        state: KeyState,
        keycode: VirtualKeyCode,
        modifierstate: ModifierState,
    ) -> Self {
        Self {
            window,
            state,
            keycode,
            modifierstate,
        }
    }
}

#[derive(Debug)]
pub struct ButtonEvent<Window> {
    pub window: Window,
    pub state: KeyState,
    pub keycode: MouseButton,
    pub modifierstate: ModifierState,
}

impl<Window> ButtonEvent<Window> {
    pub fn new(
        window: Window,
        state: KeyState,
        keycode: MouseButton,
        modifierstate: ModifierState,
    ) -> Self {
        Self {
            window,
            state,
            keycode,
            modifierstate,
        }
    }
}

#[derive(Debug)]
pub struct MotionEvent<Window> {
    pub position: [i32; 2],
    pub window: Window,
}

impl<Window> MotionEvent<Window> {
    pub fn new(position: [i32; 2], window: Window) -> Self {
        Self { position, window }
    }
}

#[derive(Debug)]
pub struct MapEvent<Window> {
    pub window: Window,
}

#[derive(Debug)]
pub struct UnmapEvent<Window> {
    pub window: Window,
}

#[derive(Debug)]
pub struct EnterEvent<Window> {
    pub window: Window,
}

#[derive(Debug)]
pub struct DestroyEvent<Window> {
    pub window: Window,
}

impl<Window> DestroyEvent<Window> {
    pub fn new(window: Window) -> Self {
        Self { window }
    }
}

#[derive(Debug)]
pub struct CreateEvent<Window> {
    pub window: Window,
    pub position: [i32; 2],
    pub size: [i32; 2],
}

impl<Window> CreateEvent<Window> {
    pub fn new(window: Window, position: [i32; 2], size: [i32; 2]) -> Self {
        Self {
            window,
            position,
            size,
        }
    }
}

#[derive(Debug)]
pub struct ConfigureEvent<Window> {
    pub window: Window,
    pub position: [i32; 2],
    pub size: [i32; 2],
}

impl<Window> ConfigureEvent<Window> {
    pub fn new(window: Window, position: [i32; 2], size: [i32; 2]) -> Self {
        Self {
            window,
            position,
            size,
        }
    }
}

#[derive(Debug)]
pub struct FullscreenEvent<Window> {
    window: Window,
    new_fullscreen: bool,
}

impl<Window> FullscreenEvent<Window> {
    pub fn new(window: Window, new_fullscreen: bool) -> Self {
        Self {
            window,
            new_fullscreen,
        }
    }
}

pub struct KeyBind {
    key: VirtualKeyCode,
    modifiers: ModifierState,
}

pub struct MouseBind {
    button: MouseButton,
    modifiers: ModifierState,
}
