use self::window_event::KeyBind;

pub mod keycodes;
pub mod window_event;
pub mod xlib;

pub trait WindowServerBackend {
    type Window;

    fn next_event(&self) -> window_event::WindowEvent<Self::Window>;
    fn add_keybind(&mut self, keybind: KeyBind, window: Option<Self::Window>);
    fn remove_keybind(
        &mut self,
        keybind: KeyBind,
        window: Option<Self::Window>,
    );
    fn add_mousebind(&mut self, keybind: KeyBind, window: Option<Self::Window>);
    fn remove_mousebind(
        &mut self,
        keybind: KeyBind,
        window: Option<Self::Window>,
    );
    fn focus_window(&self, window: Self::Window);
    fn unfocus_window(&self, window: Self::Window);
    fn move_window(&self, window: Self::Window, pos: i32);
    fn resize_window(&self, window: Self::Window, pos: i32);
    fn hide_window(&self, window: Self::Window);
    fn screen_size(&self) -> (i32, i32);
    fn kill_window(&self, window: Self::Window);
}
