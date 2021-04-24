use log::info;

use x11::xlib::{self, Window, XEvent};

use crate::{
    clients::{Client, ClientKey, ClientState},
    xlib::XLib,
};

pub struct WindowManager {
    clients: ClientState,
    xlib: XLib,
}

pub enum Direction {
    Left,
    Right,
}

impl WindowManager {
    pub fn new() -> Self {
        let clients = ClientState::with_virtualscreens(3);
        let xlib = XLib::new().init();

        Self { clients, xlib }
    }

    pub fn run(mut self) -> ! {
        loop {
            let event = self.xlib.next_event();

            match event.get_type() {
                xlib::MapRequest => self.map_request(&event),
                xlib::UnmapNotify => self.unmap_notify(&event),
                xlib::ConfigureRequest => self.configure_request(&event),
                xlib::EnterNotify => self.enter_notify(&event),
                xlib::DestroyNotify => self.destroy_notify(&event),
                xlib::ButtonPress => self.destroy_notify(&event),
                _ => {}
            }
        }
    }

    fn rotate_virtual_screen(&mut self, dir: Direction) {
        match dir {
            Direction::Left => self.clients.rotate_left(),
            Direction::Right => self.clients.rotate_right(),
        }

        self.clients
            .iter_current_screen()
            .for_each(|(_, c)| self.xlib.move_resize_client(c));

        self.clients
            .iter_hidden()
            .for_each(|(_, c)| self.xlib.hide_client(c));

        // focus first client in all visible clients
        let to_focus = self.clients.iter_visible().next().map(|(k, _)| k).cloned();

        if let Some(key) = to_focus {
            self.focus_client(&key);
        }
    }

    fn arrange_clients(&mut self) {
        let (width, height) = self.xlib.dimensions();
        self.clients.arrange_virtual_screen(width, height, None);

        self.clients
            .iter_current_screen()
            .for_each(|(_, c)| self.xlib.move_resize_client(c));
    }

    fn focus_client<K>(&mut self, key: &K)
    where
        K: ClientKey,
    {
        let (new, old) = self.clients.focus_client(key);

        if let Some(new) = new.into_option() {
            self.xlib.focus_client(new);
        }

        if let Some(old) = old.into_option() {
            self.xlib.unfocus_client(old);
        }
    }

    fn unfocus_client(&mut self) {
        if let Some(client) = self.clients.unfocus().into_option() {
            self.xlib.unfocus_client(client);
        }
    }

    fn new_client(&mut self, window: Window) {
        self.clients.insert(Client::new_default(window)).unwrap();
        self.xlib.map_window(window);

        self.focus_client(&window);

        self.arrange_clients();
    }

    fn map_request(&mut self, event: &XEvent) {
        let event = unsafe { &event.map_request };

        info!("MapRequest: {:?}", event);

        if !self.clients.contains(&event.window) {
            info!("MapRequest: new client");

            self.new_client(event.window);
        }
    }

    fn unmap_notify(&mut self, event: &XEvent) {
        let event = unsafe { &event.unmap };
        info!("UnmapNotify: {:?}", event);

        self.clients.remove(&event.window);

        self.arrange_clients();
    }

    fn destroy_notify(&mut self, event: &XEvent) {
        let event = unsafe { &event.destroy_window };
        info!("DestroyNotify: {:?}", event);

        self.clients.remove(&event.window);

        self.arrange_clients();
    }

    fn configure_request(&mut self, event: &XEvent) {
        let event = unsafe { &event.configure_request };
        info!("ConfigureRequest: {:?}", event);

        match self.clients.get(&event.window).into_option() {
            Some(client) => self.xlib.configure_client(client),
            None => self.xlib.configure_window(event),
        }
    }

    fn enter_notify(&mut self, event: &XEvent) {
        let event = unsafe { &event.crossing };
        info!("EnterNotify: {:?}", event);

        self.focus_client(&event.window);
    }

    fn button_notify(&mut self, event: &XEvent) {
        let event = unsafe { &event.button };
        info!("EnterNotify: {:?}", event);

        self.focus_client(&event.subwindow);
        if let Some(client) = self.clients.get(&event.subwindow).into_option() {
            self.xlib.raise_client(client);
        }
    }
}
