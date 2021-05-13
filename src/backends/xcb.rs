//x11 backend
#![allow(dead_code)]

use log::error;
use num_traits::FromPrimitive;
use num_traits::ToPrimitive;
use std::sync::Arc;

use x11rb::{
    connect,
    connection::Connection,
    errors::ReplyError,
    errors::ReplyOrIdError,
    protocol::xproto::{
        Atom, ChangeWindowAttributesAux, ConnectionExt, EventMask, Screen,
        Setup,
    },
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keysyms() {
        let xcb = create_backend().unwrap();

        let mapping = xcb
            .connection
            .get_keyboard_mapping(
                xcb.setup().min_keycode,
                xcb.setup().max_keycode - xcb.setup().min_keycode + 1,
            )
            .unwrap();

        let mapping = mapping.reply().unwrap();

        for (i, keysyms) in mapping
            .keysyms
            .chunks(mapping.keysyms_per_keycode as usize)
            .enumerate()
        {
            println!(
                "keycode: {:#x?}\tkeysyms: {:0x?}",
                xcb.setup().min_keycode as usize + i,
                keysyms
            );
        }
    }
}

#[repr(u8)]
#[derive(FromPrimitive, ToPrimitive)]
pub enum MouseButton {
    Left = 1,
    Middle,
    Right,
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    Backwards,
    Forwards,
}

#[derive(FromPrimitive, ToPrimitive)]
pub enum Key {
    BackSpace = 0xff08,
    Tab = 0xff09,
    Linefeed = 0xff0a,
    Clear = 0xff0b,
    Return = 0xff0d,
    Pause = 0xff13,
    ScrollLock = 0xff14,
    SysReq = 0xff15,
    Escape = 0xff1b,
    Delete = 0xffff,
    Home = 0xff50,
    Left = 0xff51,
    Up = 0xff52,
    Right = 0xff53,
    Down = 0xff54,
    PageUp = 0xff55,
    PageDown = 0xff56,
    End = 0xff57,
    Begin = 0xff58,
    Space = 0x0020,
    Exclam = 0x0021,
    Quotedbl = 0x0022,
    Numbersign = 0x0023,
    Dollar = 0x0024,
    Percent = 0x0025,
    Ampersand = 0x0026,
    Apostrophe = 0x0027,
    ParenLeft = 0x0028,
    ParenRight = 0x0029,
    Asterisk = 0x002a,
    Plus = 0x002b,
    Comma = 0x002c,
    Minus = 0x002d,
    Period = 0x002e,
    Slash = 0x002f,
    Zero = 0x0030,
    One = 0x0031,
    Two = 0x0032,
    Three = 0x0033,
    Four = 0x0034,
    Five = 0x0035,
    Six = 0x0036,
    Seven = 0x0037,
    Eight = 0x0038,
    Nine = 0x0039,
    Colon = 0x003a,
    Semicolon = 0x003b,
    Less = 0x003c,
    Equal = 0x003d,
    Greater = 0x003e,
    Question = 0x003f,
    At = 0x0040,
    UppercaseA = 0x0041,
    UppercaseB = 0x0042,
    UppercaseC = 0x0043,
    UppercaseD = 0x0044,
    UppercaseE = 0x0045,
    UppercaseF = 0x0046,
    UppercaseG = 0x0047,
    UppercaseH = 0x0048,
    UppercaseI = 0x0049,
    UppercaseJ = 0x004a,
    UppercaseK = 0x004b,
    UppercaseL = 0x004c,
    UppercaseM = 0x004d,
    UppercaseN = 0x004e,
    UppercaseO = 0x004f,
    UppercaseP = 0x0050,
    UppercaseQ = 0x0051,
    UppercaseR = 0x0052,
    UppercaseS = 0x0053,
    UppercaseT = 0x0054,
    UppercaseU = 0x0055,
    UppercaseV = 0x0056,
    UppercaseW = 0x0057,
    UppercaseX = 0x0058,
    UppercaseY = 0x0059,
    UppercaseZ = 0x005a,
    BracketLeft = 0x005b,
    Backslash = 0x005c,
    BracketRight = 0x005d,
    AsciiCircum = 0x005e,
    Underscore = 0x005f,
    Grave = 0x0060,
    LowercaseA = 0x0061,
    LowercaseB = 0x0062,
    LowercaseC = 0x0063,
    LowercaseD = 0x0064,
    LowercaseE = 0x0065,
    LowercaseF = 0x0066,
    LowercaseG = 0x0067,
    LowercaseH = 0x0068,
    LowercaseI = 0x0069,
    LowercaseJ = 0x006a,
    LowercaseK = 0x006b,
    LowercaseL = 0x006c,
    LowercaseM = 0x006d,
    LowercaseN = 0x006e,
    LowercaseO = 0x006f,
    LowercaseP = 0x0070,
    LowercaseQ = 0x0071,
    LowercaseR = 0x0072,
    LowercaseS = 0x0073,
    LowercaseT = 0x0074,
    LowercaseU = 0x0075,
    LowercaseV = 0x0076,
    LowercaseW = 0x0077,
    LowercaseX = 0x0078,
    LowercaseY = 0x0079,
    LowercaseZ = 0x007a,
    BraceLeft = 0x007b,
    Bar = 0x007c,
    BraceRight = 0x007d,
    AsciiTilde = 0x007e,
}

impl Key {}

// '<,'>s/.* XK_\([A-z,_]*\)[ ]*\(0x[a-f,0-9]*\).*/\1 = \2,/

struct Atoms {
    wm_protocols: Atom,
    wm_state: Atom,
    wm_delete_window: Atom,
    wm_take_focus: Atom,
    net_supported: Atom,
    net_active_window: Atom,
    net_client_list: Atom,
    net_wm_name: Atom,
    net_wm_state: Atom,
    net_wm_state_fullscreen: Atom,
    net_wm_window_type: Atom,
    net_wm_window_type_dialog: Atom,
}

impl Atoms {
    fn new<C>(connection: Arc<C>) -> Result<Self, ReplyOrIdError>
    where
        C: Connection,
    {
        let wm_protocols = connection.intern_atom(false, b"WM_PROTOCOLS")?;
        let wm_state = connection.intern_atom(false, b"WM_STATE")?;
        let wm_delete_window =
            connection.intern_atom(false, b"WM_DELETE_WINDOW")?;
        let wm_take_focus = connection.intern_atom(false, b"WM_TAKE_FOCUS")?;
        let net_supported = connection.intern_atom(false, b"_NET_SUPPORTED")?;
        let net_active_window =
            connection.intern_atom(false, b"_NET_ACTIVE_WINDOW")?;
        let net_client_list =
            connection.intern_atom(false, b"_NET_CLIENT_LIST")?;
        let net_wm_name = connection.intern_atom(false, b"_NET_WM_NAME")?;
        let net_wm_state = connection.intern_atom(false, b"_NET_WM_STATE")?;
        let net_wm_state_fullscreen =
            connection.intern_atom(false, b"_NET_WM_STATE_FULLSCREEN")?;
        let net_wm_window_type =
            connection.intern_atom(false, b"_NET_WM_WINDOW_TYPE")?;
        let net_wm_window_type_dialog =
            connection.intern_atom(false, b"_NET_WM_WINDOW_TYPE_DIALOG")?;

        Ok(Self {
            wm_protocols: wm_protocols.reply()?.atom,
            wm_state: wm_state.reply()?.atom,
            wm_delete_window: wm_delete_window.reply()?.atom,
            wm_take_focus: wm_take_focus.reply()?.atom,
            net_supported: net_supported.reply()?.atom,
            net_active_window: net_active_window.reply()?.atom,
            net_client_list: net_client_list.reply()?.atom,
            net_wm_name: net_wm_name.reply()?.atom,
            net_wm_state: net_wm_state.reply()?.atom,
            net_wm_state_fullscreen: net_wm_state_fullscreen.reply()?.atom,
            net_wm_window_type: net_wm_window_type.reply()?.atom,
            net_wm_window_type_dialog: net_wm_window_type_dialog.reply()?.atom,
        })
    }
}

pub struct X11Backend<C>
where
    C: Connection,
{
    connection: Arc<C>,
    screen: usize,
    atoms: Atoms,
}

pub fn create_backend(
) -> Result<X11Backend<impl Connection + Send + Sync>, Box<dyn std::error::Error>>
{
    let (connection, screen) = connect(None)?;

    Ok(X11Backend::new(Arc::new(connection), screen)?)
}

impl<C> X11Backend<C>
where
    C: Connection,
{
    pub fn new(
        connection: Arc<C>,
        screen: usize,
    ) -> Result<Self, ReplyOrIdError> {
        let atoms = Atoms::new(connection.clone())?;
        Ok(Self {
            connection,
            screen,
            atoms,
        })
    }

    fn setup(&self) -> &Setup {
        self.connection.setup()
    }

    fn screen(&self) -> &Screen {
        &self.connection.setup().roots[self.screen]
    }

    fn root(&self) -> u32 {
        self.screen().root
    }

    // this needs the mask aswell to determine the keysym
    fn keysym_for_keycode(&self, keycode: u8) -> Option<Key> {
        let setup = self.setup();
        let mapping = self
            .connection
            .get_keyboard_mapping(
                setup.min_keycode,
                setup.max_keycode - setup.min_keycode + 1,
            )
            .ok()?;

        let mapping = mapping.reply().ok()?;

        mapping
            .keysyms
            .chunks(mapping.keysyms_per_keycode as usize)
            .nth(keycode as usize)
            .and_then(|keysyms| Key::from_u32(keysyms[0]))
    }

    fn keycode_for_keysym<K>(&self, keysym: &K) -> Option<u8>
    where
        K: num_traits::ToPrimitive,
    {
        if let Some(keysym) = keysym.to_u32() {
            let setup = self.setup();
            let mapping = self
                .connection
                .get_keyboard_mapping(
                    setup.min_keycode,
                    setup.max_keycode - setup.min_keycode + 1,
                )
                .ok()?;

            let mapping = mapping.reply().ok()?;

            mapping
                .keysyms
                .chunks(mapping.keysyms_per_keycode as usize)
                .enumerate()
                .find_map(|(i, keysyms)| {
                    if keysyms.contains(&keysym) {
                        Some(setup.min_keycode + i as u8)
                    } else {
                        None
                    }
                })
        } else {
            None
        }
    }

    pub fn request_substructure_events(&self) -> Result<(), ReplyError> {
        let attributes = ChangeWindowAttributesAux::default().event_mask(
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        );

        match self
            .connection
            .change_window_attributes(self.root(), &attributes)?
            .check()
        {
            Ok(_) => Ok(()),
            Err(err) => {
                error!(
                    "Failed to request substructure redirect/notify: another \
                     window manager is running. {:#?}",
                    err
                );

                Err(err)
            }
        }
    }
}
