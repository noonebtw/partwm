#![allow(dead_code)]

use super::keycodes::{KeyOrButton, MouseButton, VirtualKeyCode};
use crate::util::{Point, Size};
use bitflags::bitflags;

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

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum KeyState {
    Pressed,
    Released,
}

#[derive(
    Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, serde::Deserialize,
)]
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

bitflags! {
    pub struct ModifierState: u32 {
        const SHIFT      =       0x01;
        const SHIFT_LOCK =      0x010;
        const CONTROL    =     0x0100;
        const ALT        =    0x01000;
        const ALT_GR     =   0x010000;
        const SUPER      =  0x0100000;
        const NUM_LOCK   = 0x01000000;
        const IGNORE_LOCK = Self::CONTROL.bits | Self::ALT.bits |
        Self::ALT_GR.bits | Self::SUPER.bits| Self::SHIFT.bits;
    }
}

impl<const N: usize> From<[ModifierKey; N]> for ModifierState {
    fn from(slice: [ModifierKey; N]) -> Self {
        let mut state = ModifierState::empty();
        for ele in slice {
            state.insert_mod(ele);
        }

        state
    }
}

impl ModifierState {
    pub fn eq_ignore_lock(&self, rhs: &Self) -> bool {
        let mask = Self::IGNORE_LOCK;
        *self & mask == *rhs & mask
    }

    pub fn with_mod(mut self, modifier: ModifierKey) -> Self {
        self.insert_mod(modifier);
        self
    }

    pub fn unset_mod(&mut self, modifier: ModifierKey) {
        self.set_mod(modifier, false);
    }

    pub fn set_mod(&mut self, modifier: ModifierKey, state: bool) {
        self.set(
            match modifier {
                ModifierKey::Shift => Self::SHIFT,
                ModifierKey::ShiftLock => Self::SHIFT_LOCK,
                ModifierKey::Control => Self::CONTROL,
                ModifierKey::Alt => Self::ALT,
                ModifierKey::AltGr => Self::ALT_GR,
                ModifierKey::Super => Self::SUPER,
                ModifierKey::NumLock => Self::NUM_LOCK,
            },
            state,
        );
    }

    pub fn insert_mod(&mut self, modifier: ModifierKey) {
        self.set_mod(modifier, true);
    }
}

impl Into<u8> for ModifierKey {
    fn into(self) -> u8 {
        self as u8
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
    pub cursor_position: Point<i32>,
    pub modifierstate: ModifierState,
}

impl<Window> ButtonEvent<Window> {
    pub fn new(
        window: Window,
        state: KeyState,
        keycode: MouseButton,
        cursor_position: Point<i32>,
        modifierstate: ModifierState,
    ) -> Self {
        Self {
            window,
            state,
            keycode,
            cursor_position,
            modifierstate,
        }
    }
}

#[derive(Debug)]
pub struct MotionEvent<Window> {
    pub position: Point<i32>,
    pub window: Window,
}

impl<Window> MotionEvent<Window> {
    pub fn new(position: Point<i32>, window: Window) -> Self {
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
    pub position: Point<i32>,
    pub size: Size<i32>,
}

impl<Window> CreateEvent<Window> {
    pub fn new(window: Window, position: Point<i32>, size: Size<i32>) -> Self {
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
    pub position: Point<i32>,
    pub size: Size<i32>,
}

impl<Window> ConfigureEvent<Window> {
    pub fn new(window: Window, position: Point<i32>, size: Size<i32>) -> Self {
        Self {
            window,
            position,
            size,
        }
    }
}

#[derive(Debug)]
pub enum FullscreenState {
    On,
    Off,
    Toggle,
}

impl From<bool> for FullscreenState {
    fn from(value: bool) -> Self {
        match value {
            true => Self::On,
            false => Self::Off,
        }
    }
}

#[derive(Debug)]
pub struct FullscreenEvent<Window> {
    pub window: Window,
    pub state: FullscreenState,
}

impl<Window> FullscreenEvent<Window> {
    pub fn new(window: Window, state: FullscreenState) -> Self {
        Self { window, state }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeyBind {
    pub key: VirtualKeyCode,
    pub modifiers: ModifierState,
}

impl KeyBind {
    pub fn new(key: VirtualKeyCode) -> Self {
        Self {
            key,
            modifiers: ModifierState::empty(),
        }
    }

    pub fn with_mod(mut self, modifier_key: ModifierKey) -> Self {
        self.modifiers.insert_mod(modifier_key);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MouseBind {
    pub button: MouseButton,
    pub modifiers: ModifierState,
}

impl MouseBind {
    pub fn new(button: MouseButton) -> Self {
        Self {
            button,
            modifiers: ModifierState::empty(),
        }
    }

    pub fn with_mod(mut self, modifier_key: ModifierKey) -> Self {
        self.modifiers.insert_mod(modifier_key);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeyOrMouseBind {
    pub key: KeyOrButton,
    pub modifiers: ModifierState,
}

impl KeyOrMouseBind {
    pub fn new(key: KeyOrButton) -> Self {
        Self {
            key,
            modifiers: ModifierState::empty(),
        }
    }

    pub fn with_mod(mut self, modifier_key: ModifierKey) -> Self {
        self.modifiers.insert_mod(modifier_key);
        self
    }
}

impl From<&KeyBind> for KeyOrMouseBind {
    fn from(keybind: &KeyBind) -> Self {
        Self {
            key: KeyOrButton::Key(keybind.key),
            modifiers: keybind.modifiers,
        }
    }
}

impl From<KeyBind> for KeyOrMouseBind {
    fn from(keybind: KeyBind) -> Self {
        Self {
            key: KeyOrButton::Key(keybind.key),
            modifiers: keybind.modifiers,
        }
    }
}

impl From<&MouseBind> for KeyOrMouseBind {
    fn from(mousebind: &MouseBind) -> Self {
        Self {
            key: KeyOrButton::Button(mousebind.button),
            modifiers: mousebind.modifiers,
        }
    }
}

impl From<MouseBind> for KeyOrMouseBind {
    fn from(mousebind: MouseBind) -> Self {
        Self {
            key: KeyOrButton::Button(mousebind.button),
            modifiers: mousebind.modifiers,
        }
    }
}
