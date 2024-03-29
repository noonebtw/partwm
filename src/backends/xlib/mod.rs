use log::{debug, error, warn};
use num_traits::Zero;
use std::{convert::TryFrom, ptr::NonNull, rc::Rc};

use thiserror::Error;

use x11::xlib::{self, Atom, Success, Window, XEvent, XKeyEvent, XA_WINDOW};

use crate::backends::{
    keycodes::KeyOrButton, xlib::keysym::mouse_button_to_xbutton,
};

use self::{
    connection::{PropMode, XLibConnection},
    ewmh::{EWMHAtom, EWMHAtoms},
    keysym::{
        keysym_to_virtual_keycode, virtual_keycode_to_keysym,
        xev_to_mouse_button, XKeySym,
    },
    wmh::{ICCCMAtom, ICCCMAtoms},
};

use super::{
    keycodes::VirtualKeyCode,
    structs::WindowType,
    window_event::{
        ButtonEvent, ConfigureEvent, DestroyEvent, EnterEvent, FullscreenEvent,
        FullscreenState, KeyEvent, KeyOrMouseBind, KeyState, MapEvent,
        ModifierState, MotionEvent, UnmapEvent, WindowEvent, WindowNameEvent,
        WindowTypeChangedEvent,
    },
    WindowServerBackend,
};
use crate::util::{Point, Size};

pub mod color;
pub mod keysym;

pub type XLibWindowEvent = WindowEvent<Window>;

#[derive(Clone)]
pub struct Display(Rc<NonNull<x11::xlib::Display>>);

#[derive(Debug, Error)]
pub enum XlibError {
    #[error("BadAccess")]
    BadAccess,
    #[error("BadAlloc")]
    BadAlloc,
    #[error("BadAtom")]
    BadAtom,
    #[error("BadColor")]
    BadColor,
    #[error("BadCursor")]
    BadCursor,
    #[error("BadDrawable")]
    BadDrawable,
    #[error("BadFont")]
    BadFont,
    #[error("BadGC")]
    BadGC,
    #[error("BadIDChoice")]
    BadIDChoice,
    #[error("BadImplementation")]
    BadImplementation,
    #[error("BadLength")]
    BadLength,
    #[error("BadMatch")]
    BadMatch,
    #[error("BadName")]
    BadName,
    #[error("BadPixmap")]
    BadPixmap,
    #[error("BadRequest")]
    BadRequest,
    #[error("BadValue")]
    BadValue,
    #[error("BadWindow")]
    BadWindow,
    #[error("Invalid XError: {0}")]
    InvalidError(u8),
}

impl From<u8> for XlibError {
    fn from(value: u8) -> Self {
        match value {
            xlib::BadAccess => XlibError::BadAccess,
            xlib::BadAlloc => XlibError::BadAlloc,
            xlib::BadAtom => XlibError::BadAtom,
            xlib::BadColor => XlibError::BadColor,
            xlib::BadCursor => XlibError::BadCursor,
            xlib::BadDrawable => XlibError::BadDrawable,
            xlib::BadFont => XlibError::BadFont,
            xlib::BadGC => XlibError::BadGC,
            xlib::BadIDChoice => XlibError::BadIDChoice,
            xlib::BadImplementation => XlibError::BadImplementation,
            xlib::BadLength => XlibError::BadLength,
            xlib::BadMatch => XlibError::BadMatch,
            xlib::BadName => XlibError::BadName,
            xlib::BadPixmap => XlibError::BadPixmap,
            xlib::BadRequest => XlibError::BadRequest,
            xlib::BadValue => XlibError::BadValue,
            xlib::BadWindow => XlibError::BadWindow,
            any => XlibError::InvalidError(any),
        }
    }
}

pub mod wmh {
    use std::{borrow::Borrow, ffi::CString, ops::Index};

    use strum::{EnumCount, EnumIter};
    use x11::xlib::Atom;

    use super::{connection::XLibConnection, Display};

    #[derive(Debug, PartialEq, Eq, EnumIter, EnumCount, Clone, Copy)]
    pub enum ICCCMAtom {
        WmName,
        WmProtocols,
        WmDeleteWindow,
        WmActiveWindow,
        WmTakeFocus,
        WmState,
        WmTransientFor,
        Utf8String,
    }

    #[derive(Debug, Clone)]
    pub struct ICCCMAtoms {
        inner: Vec<Atom>,
    }

    impl Index<ICCCMAtom> for ICCCMAtoms {
        type Output = Atom;

        fn index(&self, index: ICCCMAtom) -> &Self::Output {
            &self.inner[usize::from(index)]
        }
    }

    impl ICCCMAtoms {
        pub fn from_connection<C: Borrow<XLibConnection>>(
            con: C,
        ) -> Option<Self> {
            ICCCMAtom::try_get_atoms(con.borrow().display())
                .map(|atoms| Self { inner: atoms })
        }
    }

    impl ICCCMAtom {
        pub fn try_get_atoms(display: Display) -> Option<Vec<Atom>> {
            use strum::IntoEnumIterator;
            Self::iter()
                .map(|atom| atom.try_into_x_atom(&display))
                .collect::<Option<Vec<_>>>()
        }

        fn try_into_x_atom(self, display: &Display) -> Option<Atom> {
            let name = CString::new::<&str>(self.into()).ok()?;
            match unsafe {
                x11::xlib::XInternAtom(
                    display.get(),
                    name.as_c_str().as_ptr(),
                    0,
                )
            } {
                0 => None,
                atom => Some(atom),
            }
        }
    }

    impl From<ICCCMAtom> for usize {
        fn from(atom: ICCCMAtom) -> Self {
            atom as usize
        }
    }

    impl From<ICCCMAtom> for &str {
        fn from(atom: ICCCMAtom) -> Self {
            match atom {
                ICCCMAtom::WmName => "WM_NAME",
                ICCCMAtom::WmProtocols => "WM_PROTOCOLS",
                ICCCMAtom::WmDeleteWindow => "WM_DELETE_WINDOW",
                ICCCMAtom::WmActiveWindow => "WM_ACTIVE_WINDOW",
                ICCCMAtom::WmTakeFocus => "WM_TAKE_FOCUS",
                ICCCMAtom::WmState => "WM_STATE",
                ICCCMAtom::WmTransientFor => "WM_TRANSIENT_FOR",
                ICCCMAtom::Utf8String => "UTF8_STRING",
            }
        }
    }
}

pub mod ewmh {
    use std::{borrow::Borrow, ffi::CString, ops::Index, os::raw::c_long};

    use strum::{EnumCount, EnumIter, FromRepr};
    use x11::xlib::{Atom, XA_ATOM};

    use super::{
        connection::{PropMode, XLibConnection},
        Display,
    };

    #[derive(
        Debug, PartialEq, Eq, EnumIter, EnumCount, Clone, Copy, FromRepr,
    )]
    pub enum EWMHAtom {
        NetSupported,
        NetClientList,
        NetNumberOfDesktops,
        NetDesktopGeometry,
        NetDesktopViewport,
        NetCurrentDesktop,
        NetDesktopNames,
        NetActiveWindow,
        NetWorkarea,
        NetSupportingWmCheck,
        NetVirtualRoots,
        NetDesktopLayout,
        NetShowingDesktop,
        NetCloseWindow,
        NetMoveresizeWindow,
        NetWmMoveresize,
        NetRestackWindow,
        NetRequestFrameExtents,
        NetWmName,
        NetWmVisibleName,
        NetWmIconName,
        NetWmVisibleIconName,
        NetWmDesktop,
        NetWmWindowType,
        NetWmState,
        NetWmAllowedActions,
        NetWmStrut,
        NetWmStrutPartial,
        NetWmIconGeometry,
        NetWmIcon,
        NetWmPid,
        NetWmHandledIcons,
        NetWmUserTime,
        NetFrameExtents,
        NetWmPing,
        NetWmSyncRequest,

        // idk if these are atoms?
        NetWmWindowTypeDesktop,
        NetWmWindowTypeDock,
        NetWmWindowTypeToolbar,
        NetWmWindowTypeMenu,
        NetWmWindowTypeUtility,
        NetWmWindowTypeSplash,
        NetWmWindowTypeDialog,
        NetWmWindowTypeNormal,
        NetWmStateModal,
        NetWmStateSticky,
        NetWmStateMaximizedVert,
        NetWmStateMaximizedHorz,
        NetWmStateShaded,
        NetWmStateSkipTaskbar,
        NetWmStateSkipPager,
        NetWmStateHidden,
        NetWmStateFullscreen,
        NetWmStateAbove,
        NetWmStateBelow,
        NetWmStateDemandsAttention,
        NetWmActionMove,
        NetWmActionResize,
        NetWmActionMinimize,
        NetWmActionShade,
        NetWmActionStick,
        NetWmActionMaximizeHorz,
        NetWmActionMaximizeVert,
        NetWmActionFullscreen,
        NetWmActionChangeDesktop,
        NetWmActionClose,
    }

    #[derive(Debug, Clone)]
    pub struct EWMHAtoms {
        inner: Vec<Atom>,
    }

    impl Index<EWMHAtom> for EWMHAtoms {
        type Output = Atom;

        fn index(&self, index: EWMHAtom) -> &Self::Output {
            &self.inner[usize::from(index)]
        }
    }

    impl EWMHAtoms {
        pub fn from_connection<C: Borrow<XLibConnection>>(
            con: C,
        ) -> Option<Self> {
            EWMHAtom::try_get_atoms(con.borrow().display())
                .map(|atoms| Self { inner: atoms })
        }

        pub fn reverse_lookup(&self, atom: Atom) -> Option<EWMHAtom> {
            self.inner
                .iter()
                .position(|a| *a == atom)
                .map(|position| EWMHAtom::from_repr(position))
                .flatten()
        }

        pub fn set_supported_atoms<C: Borrow<XLibConnection>>(&self, con: C) {
            let supported_atoms = [
                self[EWMHAtom::NetActiveWindow],
                self[EWMHAtom::NetWmWindowType],
                self[EWMHAtom::NetWmWindowTypeDialog],
                self[EWMHAtom::NetWmState],
                self[EWMHAtom::NetWmName],
                self[EWMHAtom::NetClientList],
                self[EWMHAtom::NetWmStateFullscreen],
            ]
            .to_vec();

            con.borrow().change_root_property_long(
                self[EWMHAtom::NetSupported],
                XA_ATOM,
                PropMode::Replace,
                supported_atoms
                    .into_iter()
                    .map(|atom| atom as c_long)
                    .collect::<Vec<_>>(),
            );
        }
    }

    impl EWMHAtom {
        pub fn try_get_atoms(display: Display) -> Option<Vec<Atom>> {
            use strum::IntoEnumIterator;
            Self::iter()
                .map(|atom| atom.try_into_x_atom(&display))
                .collect::<Option<Vec<_>>>()
        }

        fn try_into_x_atom(self, display: &Display) -> Option<Atom> {
            let name = CString::new::<&str>(self.into()).ok()?;
            match unsafe {
                x11::xlib::XInternAtom(
                    display.get(),
                    name.as_c_str().as_ptr(),
                    0,
                )
            } {
                0 => None,
                atom => Some(atom),
            }
        }
    }

    impl From<EWMHAtom> for u8 {
        fn from(atom: EWMHAtom) -> Self {
            atom as u8
        }
    }

    impl From<EWMHAtom> for usize {
        fn from(atom: EWMHAtom) -> Self {
            atom as usize
        }
    }

    impl From<EWMHAtom> for &str {
        fn from(atom: EWMHAtom) -> Self {
            match atom {
                EWMHAtom::NetSupported => "_NET_SUPPORTED",
                EWMHAtom::NetClientList => "_NET_CLIENT_LIST",
                EWMHAtom::NetNumberOfDesktops => "_NET_NUMBER_OF_DESKTOPS",
                EWMHAtom::NetDesktopGeometry => "_NET_DESKTOP_GEOMETRY",
                EWMHAtom::NetDesktopViewport => "_NET_DESKTOP_VIEWPORT",
                EWMHAtom::NetCurrentDesktop => "_NET_CURRENT_DESKTOP",
                EWMHAtom::NetDesktopNames => "_NET_DESKTOP_NAMES",
                EWMHAtom::NetActiveWindow => "_NET_ACTIVE_WINDOW",
                EWMHAtom::NetWorkarea => "_NET_WORKAREA",
                EWMHAtom::NetSupportingWmCheck => "_NET_SUPPORTING_WM_CHECK",
                EWMHAtom::NetVirtualRoots => "_NET_VIRTUAL_ROOTS",
                EWMHAtom::NetDesktopLayout => "_NET_DESKTOP_LAYOUT",
                EWMHAtom::NetShowingDesktop => "_NET_SHOWING_DESKTOP",
                EWMHAtom::NetCloseWindow => "_NET_CLOSE_WINDOW",
                EWMHAtom::NetMoveresizeWindow => "_NET_MOVERESIZE_WINDOW",
                EWMHAtom::NetWmMoveresize => "_NET_WM_MOVERESIZE",
                EWMHAtom::NetRestackWindow => "_NET_RESTACK_WINDOW",
                EWMHAtom::NetRequestFrameExtents => {
                    "_NET_REQUEST_FRAME_EXTENTS"
                }
                EWMHAtom::NetWmName => "_NET_WM_NAME",
                EWMHAtom::NetWmVisibleName => "_NET_WM_VISIBLE_NAME",
                EWMHAtom::NetWmIconName => "_NET_WM_ICON_NAME",
                EWMHAtom::NetWmVisibleIconName => "_NET_WM_VISIBLE_ICON_NAME",
                EWMHAtom::NetWmDesktop => "_NET_WM_DESKTOP",
                EWMHAtom::NetWmWindowType => "_NET_WM_WINDOW_TYPE",
                EWMHAtom::NetWmState => "_NET_WM_STATE",
                EWMHAtom::NetWmAllowedActions => "_NET_WM_ALLOWED_ACTIONS",
                EWMHAtom::NetWmStrut => "_NET_WM_STRUT",
                EWMHAtom::NetWmStrutPartial => "_NET_WM_STRUT_PARTIAL",
                EWMHAtom::NetWmIconGeometry => "_NET_WM_ICON_GEOMETRY",
                EWMHAtom::NetWmIcon => "_NET_WM_ICON",
                EWMHAtom::NetWmPid => "_NET_WM_PID",
                EWMHAtom::NetWmHandledIcons => "_NET_WM_HANDLED_ICONS",
                EWMHAtom::NetWmUserTime => "_NET_WM_USER_TIME",
                EWMHAtom::NetFrameExtents => "_NET_FRAME_EXTENTS",
                EWMHAtom::NetWmPing => "_NET_WM_PING",
                EWMHAtom::NetWmSyncRequest => "_NET_WM_SYNC_REQUEST",
                EWMHAtom::NetWmWindowTypeDesktop => {
                    "_NET_WM_WINDOW_TYPE_DESKTOP"
                }
                EWMHAtom::NetWmWindowTypeDock => "_NET_WM_WINDOW_TYPE_DOCK",
                EWMHAtom::NetWmWindowTypeToolbar => {
                    "_NET_WM_WINDOW_TYPE_TOOLBAR"
                }
                EWMHAtom::NetWmWindowTypeMenu => "_NET_WM_WINDOW_TYPE_MENU",
                EWMHAtom::NetWmWindowTypeUtility => {
                    "_NET_WM_WINDOW_TYPE_UTILITY"
                }
                EWMHAtom::NetWmWindowTypeSplash => "_NET_WM_WINDOW_TYPE_SPLASH",
                EWMHAtom::NetWmWindowTypeDialog => "_NET_WM_WINDOW_TYPE_DIALOG",
                EWMHAtom::NetWmWindowTypeNormal => "_NET_WM_WINDOW_TYPE_NORMAL",
                EWMHAtom::NetWmStateModal => "_NET_WM_STATE_MODAL",
                EWMHAtom::NetWmStateSticky => "_NET_WM_STATE_STICKY",
                EWMHAtom::NetWmStateMaximizedVert => {
                    "_NET_WM_STATE_MAXIMIZED_VERT"
                }
                EWMHAtom::NetWmStateMaximizedHorz => {
                    "_NET_WM_STATE_MAXIMIZED_HORZ"
                }
                EWMHAtom::NetWmStateShaded => "_NET_WM_STATE_SHADED",
                EWMHAtom::NetWmStateSkipTaskbar => "_NET_WM_STATE_SKIP_TASKBAR",
                EWMHAtom::NetWmStateSkipPager => "_NET_WM_STATE_SKIP_PAGER",
                EWMHAtom::NetWmStateHidden => "_NET_WM_STATE_HIDDEN",
                EWMHAtom::NetWmStateFullscreen => "_NET_WM_STATE_FULLSCREEN",
                EWMHAtom::NetWmStateAbove => "_NET_WM_STATE_ABOVE",
                EWMHAtom::NetWmStateBelow => "_NET_WM_STATE_BELOW",
                EWMHAtom::NetWmStateDemandsAttention => {
                    "_NET_WM_STATE_DEMANDS_ATTENTION"
                }
                EWMHAtom::NetWmActionMove => "_NET_WM_ACTION_MOVE",
                EWMHAtom::NetWmActionResize => "_NET_WM_ACTION_RESIZE",
                EWMHAtom::NetWmActionMinimize => "_NET_WM_ACTION_MINIMIZE",
                EWMHAtom::NetWmActionShade => "_NET_WM_ACTION_SHADE",
                EWMHAtom::NetWmActionStick => "_NET_WM_ACTION_STICK",
                EWMHAtom::NetWmActionMaximizeHorz => {
                    "_NET_WM_ACTION_MAXIMIZE_HORZ"
                }
                EWMHAtom::NetWmActionMaximizeVert => {
                    "_NET_WM_ACTION_MAXIMIZE_VERT"
                }
                EWMHAtom::NetWmActionFullscreen => "_NET_WM_ACTION_FULLSCREEN",
                EWMHAtom::NetWmActionChangeDesktop => {
                    "_NET_WM_ACTION_CHANGE_DESKTOP"
                }
                EWMHAtom::NetWmActionClose => "_NET_WM_ACTION_CLOSE",
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn get_atoms() {
            let display = Display::open().unwrap();
            let atoms = EWMHAtom::try_get_atoms(display).expect("atoms");
            println!("{:?}", atoms);
        }
    }
}

pub mod connection {
    use std::{
        ffi::CString,
        mem::size_of,
        os::raw::{c_char, c_long},
    };

    use bytemuck::from_bytes;
    use x11::xlib::{self, Atom, Window};

    use super::{xpointer::XPointer, Display};

    pub struct XLibConnection {
        display: Display,
        root: Window,
        screen: i32,
    }

    impl Drop for XLibConnection {
        fn drop(&mut self) {
            unsafe { xlib::XCloseDisplay(self.display.get()) };
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum PropMode {
        Replace,
        Append,
        Prepend,
    }

    impl From<PropMode> for i32 {
        fn from(mode: PropMode) -> Self {
            match mode {
                PropMode::Replace => xlib::PropModeReplace,
                PropMode::Append => xlib::PropModeAppend,
                PropMode::Prepend => xlib::PropModePrepend,
            }
        }
    }

    impl XLibConnection {
        pub fn new() -> Option<Self> {
            if let Some(display) = Display::open() {
                let screen = unsafe { xlib::XDefaultScreen(display.get()) };
                let root = unsafe { xlib::XRootWindow(display.get(), screen) };

                Some(Self {
                    display,
                    root,
                    screen,
                })
            } else {
                None
            }
        }

        pub fn dpy(&self) -> *mut xlib::Display {
            self.display.get()
        }
        pub fn display(&self) -> Display {
            self.display.clone()
        }

        pub fn root(&self) -> Window {
            self.root
        }

        pub fn screen(&self) -> i32 {
            self.screen
        }

        pub fn get_window_property(
            &self,
            window: Window,
            atom: Atom,
            atom_type: Atom,
        ) -> Option<Vec<u8>> {
            let mut format_returned = 0;
            let mut items_returned = 0;
            let mut bytes_after_return = 0;
            let mut type_returned = 0;

            let (ptr, success) =
                XPointer::<u8>::build_with_result(|ptr| unsafe {
                    xlib::XGetWindowProperty(
                        self.dpy(),
                        window,
                        atom,
                        0,
                        4096 / 4,
                        0,
                        atom_type,
                        &mut type_returned,
                        &mut format_returned,
                        &mut items_returned,
                        &mut bytes_after_return,
                        ptr as *mut _ as *mut _,
                    ) == i32::from(xlib::Success)
                });

            success.then(|| ptr).flatten().map(|ptr| {
                unsafe {
                    std::slice::from_raw_parts(
                        ptr.as_ptr(),
                        items_returned as usize * format_returned as usize,
                    )
                }
                .to_vec()
            })
        }

        pub fn get_property_long(
            &self,
            window: Window,
            atom: Atom,
            atom_type: Atom,
        ) -> Option<Vec<c_long>> {
            self.get_window_property(window, atom, atom_type)
                .map(|bytes| {
                    bytes
                        .chunks(size_of::<c_long>())
                        .map(|bytes| *from_bytes::<c_long>(bytes))
                        .collect::<Vec<_>>()
                })
        }

        pub fn get_text_property(
            &self,
            window: Window,
            atom: Atom,
        ) -> Option<String> {
            unsafe {
                let mut text_prop =
                    std::mem::MaybeUninit::<xlib::XTextProperty>::zeroed()
                        .assume_init();

                if xlib::XGetTextProperty(
                    self.dpy(),
                    window,
                    &mut text_prop,
                    atom,
                ) == 0
                {
                    return None;
                }

                CString::from_raw(text_prop.value.cast::<c_char>())
                    .into_string()
                    .ok()
            }
        }

        pub fn delete_property(&self, window: Window, atom: Atom) {
            unsafe {
                xlib::XDeleteProperty(self.dpy(), window, atom);
            }
        }

        pub fn change_property_byte<T: AsRef<[u8]>>(
            &self,
            window: Window,
            atom: Atom,
            atom_type: Atom,
            mode: PropMode,
            data: T,
        ) {
            unsafe {
                xlib::XChangeProperty(
                    self.dpy(),
                    window,
                    atom,
                    atom_type,
                    8,
                    mode.into(),
                    data.as_ref().as_ptr().cast::<u8>(),
                    data.as_ref().len() as i32,
                );
            }
        }

        pub fn change_root_property_byte<T: AsRef<[u8]>>(
            &self,
            atom: Atom,
            atom_type: Atom,
            mode: PropMode,
            data: T,
        ) {
            self.change_property_byte(self.root, atom, atom_type, mode, data)
        }

        pub fn change_property_long<T: AsRef<[c_long]>>(
            &self,
            window: Window,
            atom: Atom,
            atom_type: Atom,
            mode: PropMode,
            data: T,
        ) {
            unsafe {
                xlib::XChangeProperty(
                    self.dpy(),
                    window,
                    atom,
                    atom_type,
                    32,
                    mode.into(),
                    data.as_ref().as_ptr().cast::<u8>(),
                    data.as_ref().len() as i32,
                );
            }
        }

        pub fn change_root_property_long<T: AsRef<[c_long]>>(
            &self,
            atom: Atom,
            atom_type: Atom,
            mode: PropMode,
            data: T,
        ) {
            self.change_property_long(self.root, atom, atom_type, mode, data)
        }
    }
}

impl Display {
    pub fn new(display: *mut x11::xlib::Display) -> Option<Self> {
        NonNull::new(display).map(|ptr| Self(Rc::new(ptr)))
    }

    // TODO: error communication
    pub fn open() -> Option<Self> {
        Self::new(unsafe { xlib::XOpenDisplay(std::ptr::null()) })
    }

    /// this should definitely be unsafe lmao
    pub fn get(&self) -> *mut x11::xlib::Display {
        self.0.as_ptr()
    }
}

pub struct XLib {
    connection: Rc<XLibConnection>,
    atoms: ICCCMAtoms,
    ewmh_atoms: EWMHAtoms,
    keybinds: Vec<KeyOrMouseBind>,
    active_border_color: Option<color::XftColor>,
    inactive_border_color: Option<color::XftColor>,
    wm_window: Window,
}

impl XLib {
    fn new() -> Self {
        let con =
            Rc::new(XLibConnection::new().expect("failed to open x display"));

        Self {
            connection: con.clone(),
            atoms: ICCCMAtoms::from_connection(con.clone()).expect("atoms"),
            ewmh_atoms: EWMHAtoms::from_connection(con.clone())
                .expect("ewmh atoms"),
            keybinds: Vec::new(),
            active_border_color: None,
            inactive_border_color: None,
            wm_window: unsafe {
                xlib::XCreateSimpleWindow(
                    con.dpy(),
                    con.root(),
                    0,
                    0,
                    1,
                    1,
                    0,
                    0,
                    0,
                )
            },
        }
    }

    unsafe fn init_as_wm(&self) {
        let mut window_attributes =
            std::mem::MaybeUninit::<xlib::XSetWindowAttributes>::zeroed()
                .assume_init();

        window_attributes.event_mask = xlib::SubstructureRedirectMask
            | xlib::StructureNotifyMask
            | xlib::SubstructureNotifyMask
            | xlib::EnterWindowMask
            | xlib::PointerMotionMask
            | xlib::ButtonPressMask;

        xlib::XChangeWindowAttributes(
            self.connection.dpy(),
            self.connection.root(),
            xlib::CWEventMask,
            &mut window_attributes,
        );

        xlib::XSelectInput(
            self.dpy(),
            self.connection.root(),
            window_attributes.event_mask,
        );

        xlib::XSetErrorHandler(Some(xlib_error_handler));
        xlib::XSync(self.dpy(), 0);

        self.ewmh_atoms.set_supported_atoms(self.connection.clone());
        self.connection.delete_property(
            self.connection.root(),
            self.ewmh_atoms[EWMHAtom::NetClientList],
        );

        self.connection.change_property_long(
            self.wm_window,
            self.ewmh_atoms[EWMHAtom::NetSupportingWmCheck],
            XA_WINDOW,
            PropMode::Replace,
            &[self.wm_window as i64],
        );

        self.connection.change_property_long(
            self.connection.root(),
            self.ewmh_atoms[EWMHAtom::NetSupportingWmCheck],
            XA_WINDOW,
            PropMode::Replace,
            &[self.wm_window as i64],
        );

        self.connection.change_property_byte(
            self.wm_window,
            self.ewmh_atoms[EWMHAtom::NetWmName],
            self.atoms[ICCCMAtom::Utf8String],
            PropMode::Replace,
            "nirgendwm".as_bytes(),
        );
    }

    //#[deprecated = "use `self.connection.dpy()` instead"]
    fn dpy(&self) -> *mut xlib::Display {
        self.connection.dpy()
    }

    fn next_xevent(&mut self) -> XEvent {
        let event = unsafe {
            let mut event = std::mem::MaybeUninit::<xlib::XEvent>::zeroed();
            xlib::XNextEvent(self.dpy(), event.as_mut_ptr());

            event.assume_init()
        };

        // match event.get_type() {
        //     xlib::KeyPress | xlib::KeyRelease => {
        //         self.update_modifier_state(AsRef::<xlib::XKeyEvent>::as_ref(
        //             &event,
        //         ));
        //     }
        //     _ => {}
        // }

        event
    }

    fn xevent_to_window_event(&self, event: XEvent) -> Option<XLibWindowEvent> {
        match event.get_type() {
            xlib::MapRequest => {
                let ev = unsafe { &event.map_request };
                Some(XLibWindowEvent::MapRequestEvent(MapEvent {
                    window: ev.window,
                }))
            }
            xlib::UnmapNotify => {
                let ev = unsafe { &event.unmap };
                Some(XLibWindowEvent::UnmapEvent(UnmapEvent {
                    window: ev.window,
                }))
            }
            xlib::ConfigureRequest => {
                let ev = unsafe { &event.configure_request };
                Some(XLibWindowEvent::ConfigureEvent(ConfigureEvent {
                    window: ev.window,
                    position: (ev.x, ev.y).into(),
                    size: (ev.width, ev.height).into(),
                }))
            }
            xlib::EnterNotify => {
                let ev = unsafe { &event.crossing };
                Some(XLibWindowEvent::EnterEvent(EnterEvent {
                    window: ev.window,
                }))
            }
            xlib::DestroyNotify => {
                let ev = unsafe { &event.destroy_window };
                Some(XLibWindowEvent::DestroyEvent(DestroyEvent {
                    window: ev.window,
                }))
            }
            xlib::MotionNotify => {
                let ev = unsafe { &event.motion };
                Some(XLibWindowEvent::MotionEvent(MotionEvent {
                    position: (ev.x, ev.y).into(),
                    window: ev.window,
                }))
            }
            // both ButtonPress and ButtonRelease use the XButtonEvent structure, aliased as either
            // XButtonReleasedEvent or XButtonPressedEvent
            xlib::ButtonPress | xlib::ButtonRelease => {
                let ev = unsafe { &event.button };
                let keycode = xev_to_mouse_button(ev).unwrap();
                let state = if ev.type_ == xlib::ButtonPress {
                    KeyState::Pressed
                } else {
                    KeyState::Released
                };

                Some(XLibWindowEvent::ButtonEvent(ButtonEvent::new(
                    ev.subwindow,
                    state,
                    keycode,
                    (ev.x, ev.y).into(),
                    ModifierState::from_modmask(ev.state),
                )))
            }
            xlib::KeyPress | xlib::KeyRelease => {
                let ev = unsafe { &event.key };

                let keycode =
                    keysym_to_virtual_keycode(self.keyev_to_keysym(ev).get());
                let state = if ev.type_ == xlib::KeyPress {
                    KeyState::Pressed
                } else {
                    KeyState::Released
                };

                keycode.map(|keycode| {
                    XLibWindowEvent::KeyEvent(KeyEvent::new(
                        ev.subwindow,
                        state,
                        keycode,
                        ModifierState::from_modmask(ev.state),
                    ))
                })
            }
            xlib::PropertyNotify => {
                let ev = unsafe { &event.property };

                match ev.atom {
                    atom if atom == self.ewmh_atoms[EWMHAtom::NetWmName]
                        || atom == self.atoms[ICCCMAtom::WmName] =>
                    {
                        self.get_window_name(ev.window).map(|name| {
                            XLibWindowEvent::WindowNameEvent(
                                WindowNameEvent::new(ev.window, name),
                            )
                        })
                    }
                    atom if atom
                        == self.ewmh_atoms[EWMHAtom::NetWmWindowType] =>
                    {
                        Some(XLibWindowEvent::WindowTypeChangedEvent(
                            WindowTypeChangedEvent::new(
                                ev.window,
                                self.get_window_type(ev.window),
                            ),
                        ))
                    }
                    _ => None,
                }
            }
            xlib::ClientMessage => {
                let ev = unsafe { &event.client_message };

                match ev.message_type {
                    message_type
                        if message_type
                            == self.ewmh_atoms[EWMHAtom::NetWmState] =>
                    {
                        let data = ev.data.as_longs();
                        if data[1] as u64
                            == self.ewmh_atoms[EWMHAtom::NetWmStateFullscreen]
                            || data[2] as u64
                                == self.ewmh_atoms
                                    [EWMHAtom::NetWmStateFullscreen]
                        {
                            debug!("fullscreen event");
                            Some(XLibWindowEvent::FullscreenEvent(
                                FullscreenEvent::new(
                                    ev.window,
                                    match data[0] /* as u64 */ {
                                        0 => FullscreenState::Off,
                                        1 => FullscreenState::On,
                                        2 => FullscreenState::Toggle,
                                        _ => {
                                            unreachable!()
                                        }
                                    },
                                ),
                            ))
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    #[allow(dead_code)]
    fn get_window_attributes(
        &self,
        window: Window,
    ) -> Option<xlib::XWindowAttributes> {
        let mut wa = unsafe {
            std::mem::MaybeUninit::<xlib::XWindowAttributes>::zeroed()
                .assume_init()
        };

        if unsafe {
            xlib::XGetWindowAttributes(self.dpy(), window, &mut wa) != 0
        } {
            Some(wa)
        } else {
            None
        }
    }

    fn get_atom_property(
        &self,
        window: Window,
        atom: xlib::Atom,
    ) -> Option<xpointer::XPointer<xlib::Atom>> {
        let mut di = 0;
        let mut dl0 = 0;
        let mut dl1 = 0;
        let mut da = 0;

        let (atom_out, success) =
            xpointer::XPointer::<xlib::Atom>::build_with_result(|ptr| unsafe {
                xlib::XGetWindowProperty(
                    self.dpy(),
                    window,
                    atom,
                    0,
                    std::mem::size_of::<xlib::Atom>() as i64,
                    0,
                    xlib::XA_ATOM,
                    &mut da,
                    &mut di,
                    &mut dl0,
                    &mut dl1,
                    ptr as *mut _ as *mut _,
                ) == i32::from(Success)
            });

        success.then(|| atom_out).flatten()
    }

    fn check_for_protocol(&self, window: Window, proto: xlib::Atom) -> bool {
        let mut protos: *mut xlib::Atom = std::ptr::null_mut();
        let mut num_protos: i32 = 0;

        unsafe {
            if xlib::XGetWMProtocols(
                self.dpy(),
                window,
                &mut protos,
                &mut num_protos,
            ) != 0
            {
                for i in 0..num_protos {
                    if *protos.offset(i as isize) == proto {
                        return true;
                    }
                }
            }
        }

        return false;
    }

    fn send_protocol(&self, window: Window, proto: Atom) -> bool {
        if self.check_for_protocol(window, proto) {
            let mut data = xlib::ClientMessageData::default();
            data.set_long(0, proto as i64);

            let mut event = XEvent {
                client_message: xlib::XClientMessageEvent {
                    type_: xlib::ClientMessage,
                    serial: 0,
                    display: self.dpy(),
                    send_event: 0,
                    window,
                    format: 32,
                    message_type: self.atoms[ICCCMAtom::WmProtocols],
                    data,
                },
            };

            unsafe {
                xlib::XSendEvent(
                    self.dpy(),
                    window,
                    0,
                    xlib::NoEventMask,
                    &mut event,
                );
            }

            true
        } else {
            false
        }
    }

    // #[allow(non_upper_case_globals)]
    // fn update_modifier_state(&mut self, keyevent: &XKeyEvent) {
    //     //keyevent.keycode
    //     let keysym = self.keyev_to_keysym(keyevent);

    //     use x11::keysym::*;

    //     let modifier = match keysym.get() {
    //         XK_Shift_L | XK_Shift_R => Some(ModifierKey::Shift),
    //         XK_Control_L | XK_Control_R => Some(ModifierKey::Control),
    //         XK_Alt_L | XK_Alt_R => Some(ModifierKey::Alt),
    //         XK_ISO_Level3_Shift => Some(ModifierKey::AltGr),
    //         XK_Caps_Lock => Some(ModifierKey::ShiftLock),
    //         XK_Num_Lock => Some(ModifierKey::NumLock),
    //         XK_Win_L | XK_Win_R => Some(ModifierKey::Super),
    //         XK_Super_L | XK_Super_R => Some(ModifierKey::Super),
    //         _ => None,
    //     };

    //     if let Some(modifier) = modifier {
    //         match keyevent.type_ {
    //             KeyPress => self.modifier_state.insert_mod(modifier),
    //             KeyRelease => self.modifier_state.unset_mod(modifier),
    //             _ => unreachable!("keyyevent != (KeyPress | KeyRelease)"),
    //         }
    //     }
    // }

    fn get_numlock_mask(&self) -> Option<u32> {
        unsafe {
            let modmap = xlib::XGetModifierMapping(self.dpy());
            let max_keypermod = (*modmap).max_keypermod;

            for i in 0..8 {
                for j in 0..max_keypermod {
                    if *(*modmap)
                        .modifiermap
                        .offset((i * max_keypermod + j) as isize)
                        == xlib::XKeysymToKeycode(
                            self.dpy(),
                            x11::keysym::XK_Num_Lock as u64,
                        )
                    {
                        return Some(1 << i);
                    }
                }
            }
        }

        None
    }

    fn grab_key_or_button(&self, binding: &KeyOrMouseBind, window: Window) {
        let modmask = binding.modifiers.as_modmask(self);

        let numlock_mask = self
            .get_numlock_mask()
            .expect("failed to query numlock mask.");

        let modifiers = vec![
            0,
            xlib::LockMask,
            numlock_mask,
            xlib::LockMask | numlock_mask,
        ];

        let keycode = match binding.key {
            KeyOrButton::Key(key) => self.vk_to_keycode(key),
            KeyOrButton::Button(button) => mouse_button_to_xbutton(button),
        };

        for modifier in modifiers.iter() {
            match binding.key {
                KeyOrButton::Key(_) => unsafe {
                    xlib::XGrabKey(
                        self.dpy(),
                        keycode,
                        modmask | modifier,
                        window,
                        1,
                        xlib::GrabModeAsync,
                        xlib::GrabModeAsync,
                    );
                },
                KeyOrButton::Button(_) => unsafe {
                    xlib::XGrabButton(
                        self.dpy(),
                        keycode as u32,
                        modmask | modifier,
                        window,
                        1,
                        (xlib::ButtonPressMask
                            | xlib::ButtonReleaseMask
                            | xlib::PointerMotionMask)
                            as u32,
                        xlib::GrabModeAsync,
                        xlib::GrabModeAsync,
                        0,
                        0,
                    );
                },
            }
        }
    }

    #[allow(dead_code)]
    fn ungrab_key_or_button(&self, binding: &KeyOrMouseBind, window: Window) {
        let modmask = binding.modifiers.as_modmask(self);

        let numlock_mask = self
            .get_numlock_mask()
            .expect("failed to query numlock mask.");

        let modifiers = vec![
            0,
            xlib::LockMask,
            numlock_mask,
            xlib::LockMask | numlock_mask,
        ];

        let keycode = match binding.key {
            KeyOrButton::Key(key) => self.vk_to_keycode(key),
            KeyOrButton::Button(button) => mouse_button_to_xbutton(button),
        };

        for modifier in modifiers.iter() {
            match binding.key {
                KeyOrButton::Key(_) => unsafe {
                    xlib::XUngrabKey(
                        self.dpy(),
                        keycode,
                        modmask | modifier,
                        window,
                    );
                },
                KeyOrButton::Button(_) => unsafe {
                    xlib::XUngrabButton(
                        self.dpy(),
                        keycode as u32,
                        modmask | modifier,
                        window,
                    );
                },
            }
        }
    }

    fn grab_global_keybinds(&self, window: Window) {
        for binding in self.keybinds.iter() {
            self.grab_key_or_button(binding, window);
        }
    }

    fn vk_to_keycode(&self, vk: VirtualKeyCode) -> i32 {
        unsafe {
            xlib::XKeysymToKeycode(
                self.dpy(),
                virtual_keycode_to_keysym(vk).unwrap() as u64,
            ) as i32
        }
    }

    fn keyev_to_keysym(&self, ev: &XKeyEvent) -> XKeySym {
        let keysym =
            unsafe { xlib::XLookupKeysym(ev as *const _ as *mut _, 0) };

        XKeySym::new(keysym as u32)
    }
}

trait ModifierStateExt {
    fn as_modmask(&self, xlib: &XLib) -> u32;
    fn from_modmask(modmask: u32) -> Self;
}

impl ModifierStateExt for ModifierState {
    fn as_modmask(&self, xlib: &XLib) -> u32 {
        let mut mask = 0;
        let _numlock_mask = xlib
            .get_numlock_mask()
            .expect("failed to query numlock mask");

        mask |= xlib::ShiftMask * u32::from(self.contains(Self::SHIFT));
        //mask |= xlib::LockMask * u32::from(self.contains(Self::SHIFT_LOCK));
        mask |= xlib::ControlMask * u32::from(self.contains(Self::CONTROL));
        mask |= xlib::Mod1Mask * u32::from(self.contains(Self::ALT));
        //mask |= xlib::Mod2Mask * u32::from(self.contains(Self::NUM_LOCK));
        //mask |= xlib::Mod3Mask * u32::from(self.contains(Self::ALT_GR));
        mask |= xlib::Mod4Mask * u32::from(self.contains(Self::SUPER));
        //mask |= numlock_mask * u32::from(self.contains(Self::NUM_LOCK));

        mask
    }

    fn from_modmask(modmask: u32) -> Self {
        let mut state = Self::empty();
        state.set(Self::SHIFT, (modmask & xlib::ShiftMask) != 0);
        //state.set(Self::SHIFT_LOCK, (modmask & xlib::LockMask) != 0);
        state.set(Self::CONTROL, (modmask & xlib::ControlMask) != 0);
        state.set(Self::ALT, (modmask & xlib::Mod1Mask) != 0);
        //state.set(Self::NUM_LOCK, (modmask & xlib::Mod2Mask) != 0);
        state.set(Self::ALT_GR, (modmask & xlib::Mod3Mask) != 0);
        state.set(Self::SUPER, (modmask & xlib::Mod4Mask) != 0);

        state
    }
}

impl WindowServerBackend for XLib {
    type Window = Window;

    fn build() -> Self {
        let xlib = Self::new();
        unsafe { xlib.init_as_wm() };
        xlib
    }

    fn next_event(&mut self) -> super::window_event::WindowEvent<Self::Window> {
        loop {
            let ev = self.next_xevent();
            let ev = self.xevent_to_window_event(ev);

            if let Some(ev) = ev {
                self.handle_event(ev.clone());
                return ev;
            }
        }
    }

    fn handle_event(
        &mut self,
        event: super::window_event::WindowEvent<Self::Window>,
    ) {
        match event {
            WindowEvent::MapRequestEvent(event) => {
                unsafe {
                    xlib::XMapWindow(self.dpy(), event.window);

                    xlib::XSelectInput(
                        self.dpy(),
                        event.window,
                        xlib::EnterWindowMask
                            | xlib::FocusChangeMask
                            | xlib::PropertyChangeMask
                            | xlib::StructureNotifyMask,
                    );
                }

                self.grab_global_keybinds(event.window);

                // add window to client list
                self.connection.change_root_property_long(
                    self.ewmh_atoms[EWMHAtom::NetClientList],
                    XA_WINDOW,
                    PropMode::Append,
                    &[event.window as i64],
                );
            }
            WindowEvent::DestroyEvent(event) => {
                self.connection
                    .get_property_long(
                        self.connection.root(),
                        self.ewmh_atoms[EWMHAtom::NetClientList],
                        XA_WINDOW,
                    )
                    .map(|mut clients| {
                        clients
                            .retain(|&window| window as Window != event.window);

                        self.connection.change_property_long(
                            self.connection.root(),
                            self.ewmh_atoms[EWMHAtom::NetClientList],
                            XA_WINDOW,
                            PropMode::Replace,
                            &clients,
                        );
                    });
            }
            _ => {}
        }
    }

    fn add_keybind(&mut self, keybind: super::window_event::KeyOrMouseBind) {
        self.grab_key_or_button(&keybind, self.connection.root());
        self.keybinds.push(keybind);
    }

    fn remove_keybind(
        &mut self,
        keybind: &super::window_event::KeyOrMouseBind,
    ) {
        self.keybinds.retain(|kb| kb != keybind);
    }

    fn focus_window(&self, window: Self::Window) {
        unsafe {
            xlib::XSetInputFocus(
                self.dpy(),
                window,
                xlib::RevertToPointerRoot,
                xlib::CurrentTime,
            );

            let border_color = self
                .active_border_color
                .as_ref()
                .map(|color| color.pixel())
                .unwrap_or_else(|| {
                    xlib::XDefaultScreenOfDisplay(self.dpy())
                        .as_ref()
                        .unwrap()
                        .white_pixel
                });

            xlib::XSetWindowBorder(self.dpy(), window, border_color);

            xlib::XChangeProperty(
                self.dpy(),
                self.connection.root(),
                self.atoms[ICCCMAtom::WmActiveWindow],
                xlib::XA_WINDOW,
                32,
                xlib::PropModeReplace,
                &window as *const u64 as *const _,
                1,
            );
        }

        self.send_protocol(window, self.atoms[ICCCMAtom::WmTakeFocus]);
    }

    fn unfocus_window(&self, window: Self::Window) {
        unsafe {
            xlib::XSetInputFocus(
                self.dpy(),
                self.connection.root(),
                xlib::RevertToPointerRoot,
                xlib::CurrentTime,
            );

            // TODO: make painting the window border a seperate function, and configurable

            let border_color = self
                .inactive_border_color
                .as_ref()
                .map(|color| color.pixel())
                .unwrap_or_else(|| {
                    xlib::XDefaultScreenOfDisplay(self.dpy())
                        .as_ref()
                        .unwrap()
                        .black_pixel
                });

            xlib::XSetWindowBorder(self.dpy(), window, border_color);

            xlib::XDeleteProperty(
                self.dpy(),
                self.connection.root(),
                self.atoms[ICCCMAtom::WmActiveWindow],
            );
        }
    }

    fn raise_window(&self, window: Self::Window) {
        unsafe {
            xlib::XRaiseWindow(self.dpy(), window);
        }
    }

    fn hide_window(&self, window: Self::Window) {
        let screen_size = self.screen_size() + Size::new(100, 100);
        self.move_window(window, screen_size.into());
    }

    fn kill_window(&self, window: Self::Window) {
        if !self.send_protocol(window, self.atoms[ICCCMAtom::WmDeleteWindow]) {
            unsafe {
                xlib::XKillClient(self.dpy(), window);
            }
        }
    }

    fn get_parent_window(&self, window: Self::Window) -> Option<Self::Window> {
        let mut parent_window: Self::Window = 0;
        if unsafe {
            xlib::XGetTransientForHint(self.dpy(), window, &mut parent_window)
                != 0
        } {
            Some(parent_window)
        } else {
            None
        }
    }

    fn configure_window(
        &self,
        window: Self::Window,
        new_size: Option<crate::util::Size<i32>>,
        new_pos: Option<crate::util::Point<i32>>,
        new_border: Option<i32>,
    ) {
        let position = new_pos.unwrap_or(Point::zero());
        let size = new_size.unwrap_or(Size::zero());
        let mut wc = xlib::XWindowChanges {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
            border_width: new_border.unwrap_or(0),
            sibling: 0,
            stack_mode: 0,
        };

        let mask = {
            let mut mask = 0;
            if new_pos.is_some() {
                mask |= xlib::CWX | xlib::CWY;
            }
            if new_size.is_some() && wc.width > 1 && wc.height > 1 {
                mask |= xlib::CWWidth | xlib::CWHeight;
            }
            if new_border.is_some() {
                mask |= xlib::CWBorderWidth;
            }

            u32::from(mask)
        };

        unsafe {
            xlib::XConfigureWindow(self.dpy(), window, mask, &mut wc);
        }
    }

    fn screen_size(&self) -> Size<i32> {
        unsafe {
            let mut wa =
                std::mem::MaybeUninit::<xlib::XWindowAttributes>::zeroed();

            xlib::XGetWindowAttributes(
                self.dpy(),
                self.connection.root(),
                wa.as_mut_ptr(),
            );

            let wa = wa.assume_init();

            (wa.width, wa.height).into()
        }
    }

    fn get_window_size(&self, window: Self::Window) -> Option<Size<i32>> {
        self.get_window_attributes(window)
            .map(|wa| (wa.width, wa.height).into())
    }

    fn grab_cursor(&self) {
        unsafe {
            xlib::XGrabPointer(
                self.dpy(),
                self.connection.root(),
                0,
                (xlib::ButtonPressMask
                    | xlib::ButtonReleaseMask
                    | xlib::PointerMotionMask) as u32,
                xlib::GrabModeAsync,
                xlib::GrabModeAsync,
                0,
                0,
                xlib::CurrentTime,
            );
        }
    }

    fn ungrab_cursor(&self) {
        unsafe {
            xlib::XUngrabPointer(self.dpy(), xlib::CurrentTime);
        }
    }

    fn move_cursor(&self, window: Option<Self::Window>, position: Point<i32>) {
        unsafe {
            xlib::XWarpPointer(
                self.dpy(),
                0,
                window.unwrap_or(self.connection.root()),
                0,
                0,
                0,
                0,
                position.x,
                position.y,
            );
        }
    }

    fn all_windows(&self) -> Option<Vec<Self::Window>> {
        let mut parent = 0;
        let mut root = 0;
        let mut children = std::ptr::null_mut();
        let mut num_children = 0;

        unsafe {
            xlib::XQueryTree(
                self.dpy(),
                self.connection.root(),
                &mut root,
                &mut parent,
                &mut children,
                &mut num_children,
            ) != 0
        }
        .then(|| {
            let windows = unsafe {
                std::slice::from_raw_parts(children, num_children as usize)
                    .to_vec()
            };

            unsafe { xlib::XFree(children as *mut _) };

            windows
        })
    }

    fn set_active_window_border_color(&mut self, color_name: &str) {
        self.active_border_color = color::XftColor::new(
            self.connection.display(),
            self.connection.screen(),
            color_name.to_owned(),
        )
        .ok();
    }

    fn set_inactive_window_border_color(&mut self, color_name: &str) {
        self.inactive_border_color = color::XftColor::new(
            self.connection.display(),
            self.connection.screen(),
            color_name.to_owned(),
        )
        .ok();
    }

    fn get_window_name(&self, window: Self::Window) -> Option<String> {
        self.connection
            .get_text_property(window, self.ewmh_atoms[EWMHAtom::NetWmName])
            .or_else(|| {
                self.connection
                    .get_text_property(window, self.atoms[ICCCMAtom::WmName])
            })
    }

    fn get_window_type(
        &self,
        window: Self::Window,
    ) -> super::structs::WindowType {
        match self
            .get_atom_property(
                window,
                self.ewmh_atoms[EWMHAtom::NetWmWindowType],
            )
            .and_then(|atom| self.ewmh_atoms.reverse_lookup(*atom))
            .and_then(|atom| WindowType::try_from(atom).ok())
        {
            Some(window_type) => window_type,
            None => match self.get_parent_window(window) {
                Some(_) => WindowType::Dialog,
                None => WindowType::Normal,
            },
        }
    }
}

impl TryFrom<EWMHAtom> for WindowType {
    type Error = ();

    fn try_from(value: EWMHAtom) -> Result<Self, Self::Error> {
        match value {
            EWMHAtom::NetWmWindowTypeDesktop => Ok(Self::Desktop),
            EWMHAtom::NetWmWindowTypeDock => Ok(Self::Dock),
            EWMHAtom::NetWmWindowTypeUtility => Ok(Self::Utility),
            EWMHAtom::NetWmWindowTypeMenu => Ok(Self::Menu),
            EWMHAtom::NetWmWindowTypeToolbar => Ok(Self::Toolbar),
            EWMHAtom::NetWmWindowTypeSplash => Ok(Self::Splash),
            EWMHAtom::NetWmWindowTypeDialog => Ok(Self::Dialog),
            EWMHAtom::NetWmWindowTypeNormal => Ok(Self::Normal),
            _ => Err(()),
        }
    }
}

#[allow(dead_code)]
unsafe extern "C" fn xlib_error_handler(
    _dpy: *mut x11::xlib::Display,
    ee: *mut x11::xlib::XErrorEvent,
) -> std::os::raw::c_int {
    let err_event = ee.as_ref().unwrap();
    let err = XlibError::from(err_event.error_code);

    match err {
        err @ XlibError::BadAccess
        | err @ XlibError::BadMatch
        | err @ XlibError::BadWindow
        | err @ XlibError::BadDrawable => {
            warn!("{:?}", err);
            0
        }
        _ => {
            error!(
                "wm: fatal error:\nrequest_code: {}\nerror_code: {}",
                err_event.request_code, err_event.error_code
            );
            std::process::exit(1)
        }
    }
}

pub mod xpointer {
    use std::{
        ops::{Deref, DerefMut},
        ptr::{null, NonNull},
    };

    use x11::xlib::XFree;

    #[repr(C)]
    #[derive(Debug)]
    pub struct XPointer<T>(NonNull<T>);

    impl<T> XPointer<T> {
        pub fn build_with<F>(cb: F) -> Option<Self>
        where
            F: FnOnce(&mut *const ()),
        {
            let mut ptr = null() as *const ();
            cb(&mut ptr);
            NonNull::new(ptr as *mut T).map(|ptr| Self(ptr))
        }

        pub fn build_with_result<F, R>(cb: F) -> (Option<Self>, R)
        where
            F: FnOnce(&mut *const ()) -> R,
        {
            let mut ptr = null() as *const ();
            let result = cb(&mut ptr);
            (NonNull::new(ptr as *mut T).map(|ptr| Self(ptr)), result)
        }

        pub fn as_ptr(&self) -> *const T {
            self.0.as_ptr() as *const _
        }
    }

    impl<T> AsRef<T> for XPointer<T> {
        fn as_ref(&self) -> &T {
            &**self
        }
    }

    impl<T> AsMut<T> for XPointer<T> {
        fn as_mut(&mut self) -> &mut T {
            &mut **self
        }
    }

    impl<T> PartialEq for XPointer<T>
    where
        T: PartialEq,
    {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }

    impl<T> Eq for XPointer<T> where T: Eq {}

    impl<T> Deref for XPointer<T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            unsafe { self.0.as_ref() }
        }
    }

    impl<T> DerefMut for XPointer<T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            unsafe { self.0.as_mut() }
        }
    }

    impl<T> Drop for XPointer<T> {
        fn drop(&mut self) {
            unsafe { XFree(self.0.as_ptr() as _) };
        }
    }
}
