#![allow(dead_code)]

use super::keycodes::{MouseButton, VirtualKeyCode};
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
            state.set_mod(ele);
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
        self.set_mod(modifier);
        self
    }

    pub fn unset_mod(&mut self, modifier: ModifierKey) {
        match modifier {
            ModifierKey::Shift => self.remove(Self::SHIFT),
            ModifierKey::ShiftLock => self.remove(Self::SHIFT_LOCK),
            ModifierKey::Control => self.remove(Self::CONTROL),
            ModifierKey::Alt => self.remove(Self::ALT),
            ModifierKey::AltGr => self.remove(Self::ALT_GR),
            ModifierKey::Super => self.remove(Self::SUPER),
            ModifierKey::NumLock => self.remove(Self::NUM_LOCK),
        }
    }

    pub fn set_mod(&mut self, modifier: ModifierKey) {
        match modifier {
            ModifierKey::Shift => self.insert(Self::SHIFT),
            ModifierKey::ShiftLock => self.insert(Self::SHIFT_LOCK),
            ModifierKey::Control => self.insert(Self::CONTROL),
            ModifierKey::Alt => self.insert(Self::ALT),
            ModifierKey::AltGr => self.insert(Self::ALT_GR),
            ModifierKey::Super => self.insert(Self::SUPER),
            ModifierKey::NumLock => self.insert(Self::NUM_LOCK),
        }
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

#[derive(Debug, Clone)]
pub struct KeyBind {
    key: VirtualKeyCode,
    modifiers: ModifierState,
}

impl KeyBind {
    pub fn new(key: VirtualKeyCode) -> Self {
        Self {
            key,
            modifiers: ModifierState::empty(),
        }
    }

    pub fn with_mod(mut self, modifier_key: ModifierKey) -> Self {
        self.modifiers.set_mod(modifier_key);
        self
    }
}

pub struct MouseBind {
    button: MouseButton,
    modifiers: ModifierState,
}
