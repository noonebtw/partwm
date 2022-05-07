use super::window_event::{self, KeyOrMouseBind};
use crate::util::{Point, Size};

pub trait WindowServerBackend {
    type Window;
    //type WindowEvent = super::window_event::WindowEvent<Self::Window>;

    fn build() -> Self;

    fn next_event(&mut self) -> window_event::WindowEvent<Self::Window>;
    fn handle_event(&mut self, event: window_event::WindowEvent<Self::Window>);

    /// adds a keybind to the specified `window`, or globally if `window` is `None`.
    /// add global keybind
    fn add_keybind(&mut self, keybind: KeyOrMouseBind);
    fn remove_keybind(&mut self, keybind: &KeyOrMouseBind);

    fn focus_window(&self, window: Self::Window);
    fn unfocus_window(&self, window: Self::Window);
    fn raise_window(&self, window: Self::Window);
    fn hide_window(&self, window: Self::Window);
    fn kill_window(&self, window: Self::Window);
    fn get_parent_window(&self, window: Self::Window) -> Option<Self::Window>;
    fn configure_window(
        &self,
        window: Self::Window,
        new_size: Option<Size<i32>>,
        new_pos: Option<Point<i32>>,
        new_border: Option<i32>,
    );

    fn screen_size(&self) -> Size<i32>;
    fn get_window_size(&self, window: Self::Window) -> Option<Size<i32>>;
    fn get_window_name(&self, window: Self::Window) -> Option<String>;

    fn grab_cursor(&self);
    fn ungrab_cursor(&self);
    fn move_cursor(&self, window: Option<Self::Window>, position: Point<i32>);

    fn all_windows(&self) -> Option<Vec<Self::Window>>;

    fn set_active_window_border_color(&mut self, color_name: &str);
    fn set_inactive_window_border_color(&mut self, color_name: &str);

    fn resize_window(&self, window: Self::Window, new_size: Size<i32>) {
        self.configure_window(window, Some(new_size), None, None);
    }

    fn move_window(&self, window: Self::Window, new_pos: Point<i32>) {
        self.configure_window(window, None, Some(new_pos), None);
    }
}
