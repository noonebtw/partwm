use self::window_event::KeyBind;

pub mod keycodes;
pub mod window_event;
pub mod xlib;

pub trait WindowServerBackend {
    type Window;

    fn new() -> Self;

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
    fn move_window(&self, window: Self::Window, new_pos: (i32, i32));
    fn resize_window(&self, window: Self::Window, new_size: (i32, i32));
    fn raise_window(&self, window: Self::Window);
    fn get_parent_window(&self, window: Self::Window) -> Option<Self::Window>;
    fn hide_window(&self, window: Self::Window);
    fn screen_size(&self) -> (i32, i32);
    fn kill_window(&self, window: Self::Window);
    fn get_window_size(&self, window: Self::Window) -> Option<(i32, i32)>;
}
