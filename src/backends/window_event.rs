#![allow(dead_code)]
use x11::xlib::Window;

use super::keycodes::{MouseButton, VirtualKeyCode};

#[derive(Debug)]
pub enum WindowEvent {
    KeyEvent {
        window: Window,
        event: KeyEvent,
    },
    ButtonEvent {
        window: Window,
        event: ButtonEvent,
    },
    MotionEvent {
        window: Window,
        event: MotionEvent,
    },
    MapRequestEvent {
        window: Window,
        event: MapEvent,
    },
    MapEvent {
        window: Window,
        event: MapEvent,
    },
    UnmapEvent {
        window: Window,
        event: UnmapEvent,
    },
    CreateEvent {
        window: Window,
        event: CreateEvent,
    },
    DestroyEvent {
        window: Window,
        event: DestroyEvent,
    },
    EnterEvent {
        window: Window,
        event: EnterEvent,
    },
    ConfigureEvent {
        window: Window,
        event: ConfigureEvent,
    },
    FullscreenEvent {
        window: Window,
        event: FullscreenEvent,
    }, //1 { window: Window, event: 1 },
}

#[derive(Debug)]
pub enum KeyState {
    Pressed,
    Released,
}

#[derive(
    Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy,
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
pub struct KeyEvent {
    state: KeyState,
    keycode: VirtualKeyCode,
    modifierstate: ModifierState,
}

impl KeyEvent {
    pub fn new(
        state: KeyState,
        keycode: VirtualKeyCode,
        modifierstate: ModifierState,
    ) -> Self {
        Self {
            state,
            keycode,
            modifierstate,
        }
    }
}

#[derive(Debug)]
pub struct ButtonEvent {
    state: KeyState,
    keycode: MouseButton,
    modifierstate: ModifierState,
}

impl ButtonEvent {
    pub fn new(
        state: KeyState,
        keycode: MouseButton,
        modifierstate: ModifierState,
    ) -> Self {
        Self {
            state,
            keycode,
            modifierstate,
        }
    }
}

#[derive(Debug)]
pub struct MotionEvent {
    position: [i32; 2],
}

impl MotionEvent {
    pub fn new(position: [i32; 2]) -> Self {
        Self { position }
    }
}

#[derive(Debug)]
pub struct MapEvent {
    window: Window,
}

impl MapEvent {
    pub fn new(window: Window) -> Self {
        Self { window }
    }
}

#[derive(Debug)]
pub struct UnmapEvent {
    window: Window,
}

impl UnmapEvent {
    pub fn new(window: Window) -> Self {
        Self { window }
    }
}

#[derive(Debug)]
pub struct EnterEvent {}

#[derive(Debug)]
pub struct DestroyEvent {
    window: Window,
}

impl DestroyEvent {
    pub fn new(window: Window) -> Self {
        Self { window }
    }
}

#[derive(Debug)]
pub struct CreateEvent {
    window: Window,
    position: [i32; 2],
    size: [i32; 2],
}

impl CreateEvent {
    pub fn new(
        window: Window,
        position: [i32; 2],
        size: [i32; 2],
    ) -> Self {
        Self {
            window,
            position,
            size,
        }
    }
}

#[derive(Debug)]
pub struct ConfigureEvent {
    pub window: Window,
    pub position: [i32; 2],
    pub size: [i32; 2],
}

impl ConfigureEvent {
    pub fn new(
        window: Window,
        position: [i32; 2],
        size: [i32; 2],
    ) -> Self {
        Self {
            window,
            position,
            size,
        }
    }
}

#[derive(Debug)]
pub struct FullscreenEvent {
    new_fullscreen: bool,
}

impl FullscreenEvent {
    pub fn new(new_fullscreen: bool) -> Self {
        Self { new_fullscreen }
    }
}
