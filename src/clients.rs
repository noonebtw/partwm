#![allow(dead_code)]

use std::collections::HashMap;
use std::num::NonZeroI32;

use log::{debug, error};

use crate::util::BuildIdentityHasher;

mod client {
    use std::hash::{Hash, Hasher};

    use x11::xlib::Window;

    #[derive(Clone, Debug)]
    pub struct Client {
        pub(crate) window: Window,
        pub(crate) size: (i32, i32),
        pub(crate) position: (i32, i32),
    }

    impl Default for Client {
        fn default() -> Self {
            Self {
                window: 0,
                size: (100, 100),
                position: (0, 0),
            }
        }
    }

    impl Client {
        pub fn new(window: Window, size: (i32, i32), position: (i32, i32)) -> Self {
            Self {
                window,
                size,
                position,
            }
        }

        pub fn new_default(window: Window) -> Self {
            Self {
                window,
                ..Default::default()
            }
        }
    }

    impl Hash for Client {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.window.hash(state);
        }
    }

    impl PartialEq for Client {
        fn eq(&self, other: &Self) -> bool {
            self.window == other.window
        }
    }

    impl Eq for Client {}

    pub trait ClientKey {
        fn key(&self) -> u64;
    }

    impl<'a> PartialEq for (dyn ClientKey + 'a) {
        fn eq(&self, other: &Self) -> bool {
            self.key() == other.key()
        }
    }

    impl<'a> Eq for (dyn ClientKey + 'a) {}

    impl<'a> Hash for (dyn ClientKey + 'a) {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.key().hash(state);
        }
    }

    impl ClientKey for Client {
        fn key(&self) -> u64 {
            self.window
        }
    }

    impl ClientKey for Window {
        fn key(&self) -> u64 {
            self.to_owned()
        }
    }

    /*
    impl Borrow<Window> for Client {
    fn borrow(&self) -> &Window {
    &self.window
    }
    }

    impl ClientKey for Rc<Client> {
    fn key(&self) -> u64 {
    self.window
    }
    }
    impl<'a> Borrow<dyn ClientKey + 'a> for Client {
    fn borrow(&self) -> &(dyn ClientKey + 'a) {
        self
    }
    }

    impl<'a> Borrow<dyn ClientKey + 'a> for Rc<Client> {
    fn borrow(&self) -> &(dyn ClientKey + 'a) {
        self
    }
    }
    */
}

pub use client::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_lists_test() {
        let mut clients = ClientState::with_virtualscreens(3);

        clients.insert(Client {
            window: 1,
            size: (1, 1),
            position: (1, 1),
        });

        clients.insert(Client {
            window: 2,
            size: (1, 1),
            position: (1, 1),
        });

        clients.arrange_virtual_screen(600, 400, None);

        println!("{:#?}", clients);

        clients
            .iter_current_screen()
            .for_each(|c| println!("{:?}", c));

        clients.remove(&1u64);

        clients.arrange_virtual_screen(600, 400, None);

        println!("{:#?}", clients);

        clients.rotate_right();

        clients.insert(Client {
            window: 3,
            size: (1, 1),
            position: (1, 1),
        });

        clients.arrange_virtual_screen(600, 400, None);

        println!("{:#?}", clients);

        clients.toggle_floating(&2u64);

        clients.rotate_left();

        clients.arrange_virtual_screen(600, 400, None);

        println!("{:#?}", clients);
    }
}

use std::{collections::VecDeque, iter::repeat};

type Clients = HashMap<u64, Client, BuildIdentityHasher>;
type ClientRef = u64;
type ClientRefs = Vec<ClientRef>;

#[derive(Debug)]
/// Used to wrap a `&` or `&mut` to a Client type.
pub enum ClientEntry<T> {
    /// Entry of a tiled client in the `ClientList`
    Tiled(T),
    /// Entry of a floating client in the `ClientList`
    Floating(T),
    /// `None` variant equivalent
    Vacant,
}

impl<T> Into<Option<T>> for ClientEntry<T> {
    fn into(self) -> Option<T> {
        match self {
            Self::Vacant => None,
            Self::Tiled(client) | Self::Floating(client) => Some(client),
        }
    }
}

impl<T> ClientEntry<T> {
    pub fn into_option(self) -> Option<T> {
        self.into()
    }

    pub fn unwrap(self) -> T {
        self.into_option().unwrap()
    }

    pub fn is_vacant(&self) -> bool {
        match self {
            ClientEntry::Vacant => true,
            _ => false,
        }
    }

    pub fn is_floating(&self) -> bool {
        match self {
            ClientEntry::Floating(_) => true,
            _ => false,
        }
    }

    pub fn is_tiled(&self) -> bool {
        match self {
            ClientEntry::Tiled(_) => true,
            _ => false,
        }
    }

    pub fn is_occupied(&self) -> bool {
        !self.is_vacant()
    }
}

#[derive(Debug, Clone)]
pub struct ClientState {
    pub(self) clients: Clients,
    pub(self) floating_clients: Clients,
    focused: Option<ClientRef>,
    pub(self) virtual_screens: VecDeque<VirtualScreen>,
}

#[derive(Debug, Clone)]
struct VirtualScreen {
    master: ClientRefs,
    aux: ClientRefs,
}

impl Default for ClientState {
    fn default() -> Self {
        let mut vss = VecDeque::<VirtualScreen>::new();
        vss.resize_with(1, Default::default);

        Self {
            clients: Default::default(),
            floating_clients: Default::default(),
            focused: None,
            virtual_screens: vss,
        }
    }
}

impl ClientState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_virtualscreens(num: usize) -> Self {
        let mut vss = VecDeque::<VirtualScreen>::new();
        vss.resize_with(num, Default::default);

        Self {
            virtual_screens: vss,
            ..Default::default()
        }
    }

    pub fn insert(&mut self, client: Client) -> Option<&Client> {
        let key = client.key();

        self.clients.insert(key, client);

        if let Some(vs) = self.virtual_screens.front_mut() {
            vs.insert(&key);
        }

        self.focus_client(&key);

        self.clients.get(&key)
    }

    pub fn remove<K>(&mut self, key: &K)
    where
        K: ClientKey,
    {
        self.remove_from_virtual_screens(key);

        self.clients.remove(&key.key());
        self.floating_clients.remove(&key.key());
    }

    pub fn contains<K>(&self, key: &K) -> bool
    where
        K: ClientKey,
    {
        let key = key.key();

        self.clients.contains_key(&key) || self.floating_clients.contains_key(&key)
    }

    pub fn iter_floating(&self) -> impl Iterator<Item = (&u64, &Client)> {
        self.floating_clients.iter()
    }

    pub fn iter_hidden(&self) -> impl Iterator<Item = (&u64, &Client)> {
        self.clients
            .iter()
            .filter(move |&(k, _)| !self.virtual_screens.front().unwrap().contains(k))
    }

    pub fn iter_visible(&self) -> impl Iterator<Item = (&u64, &Client)> {
        self.iter_floating().chain(self.iter_current_screen())
    }

    pub fn iter_current_screen(&self) -> impl Iterator<Item = (&u64, &Client)> {
        self.clients
            .iter()
            .filter(move |&(k, _)| self.virtual_screens.front().unwrap().contains(k))
    }

    pub fn get<K>(&self, key: &K) -> ClientEntry<&Client>
    where
        K: ClientKey,
    {
        match self.clients.get(&key.key()) {
            Some(client) => ClientEntry::Tiled(client),
            None => match self.floating_clients.get(&key.key()) {
                Some(client) => ClientEntry::Floating(client),
                None => ClientEntry::Vacant,
            },
        }
    }

    pub fn get_mut<K>(&mut self, key: &K) -> ClientEntry<&mut Client>
    where
        K: ClientKey,
    {
        match self.clients.get_mut(&key.key()) {
            Some(client) => ClientEntry::Tiled(client),
            None => match self.floating_clients.get_mut(&key.key()) {
                Some(client) => ClientEntry::Floating(client),
                None => ClientEntry::Vacant,
            },
        }
    }

    pub fn get_focused(&self) -> ClientEntry<&Client> {
        if let Some(focused) = self.focused {
            self.get(&focused)
        } else {
            ClientEntry::Vacant
        }
    }

    pub fn rotate_right(&mut self) {
        self.virtual_screens.rotate_right(1);
    }

    pub fn rotate_left(&mut self) {
        self.virtual_screens.rotate_left(1);
    }

    /**
    Sets a tiled client to floating and returns true, does nothing for a floating client and
    returns false. If this function returns `true` you have to call `arrange_clients` after.
    */
    pub fn set_floating<K>(&mut self, key: &K) -> bool
    where
        K: ClientKey,
    {
        if self.get(key).is_tiled() {
            self.toggle_floating(key);

            true
        } else {
            false
        }
    }

    /**
    This function invalidates the tiling, call `arrange_clients` to fix it again (it doesn't do it
    automatically since xlib has to move and resize all windows anyways).
    */
    pub fn toggle_floating<K>(&mut self, key: &K)
    where
        K: ClientKey,
    {
        let key = key.key();
        let client = self.clients.remove(&key);
        let floating_client = self.floating_clients.remove(&key);

        match (client, floating_client) {
            (Some(client), None) => {
                self.floating_clients.insert(key, client);
                self.remove_from_virtual_screens(&key);
            }
            (None, Some(client)) => {
                self.clients.insert(key, client);
                if let Some(vs) = self.virtual_screens.front_mut() {
                    vs.insert(&key);
                }
            }
            _ => {
                error!("wtf? Client was present in tiled and floating list.")
            }
        };
    }

    fn remove_from_virtual_screens<K>(&mut self, key: &K)
    where
        K: ClientKey,
    {
        if let Some(vs) = self.get_mut_virtualscreen_for_client(key) {
            vs.remove(key);
        }
    }

    fn get_virtualscreen_for_client<K>(&self, key: &K) -> Option<&VirtualScreen>
    where
        K: ClientKey,
    {
        self.virtual_screens
            .iter()
            .find_map(|vs| if vs.contains(key) { Some(vs) } else { None })
    }

    fn get_mut_virtualscreen_for_client<K>(&mut self, key: &K) -> Option<&mut VirtualScreen>
    where
        K: ClientKey,
    {
        self.virtual_screens.iter_mut().find_map(
            |vs| {
                if vs.contains(key) {
                    Some(vs)
                } else {
                    None
                }
            },
        )
    }

    /**
    focuses client `key` if it contains `key` and returns a reference to the  newly and the previously
    focused clients if any.
    */
    pub fn focus_client<K>(&mut self, key: &K) -> (ClientEntry<&Client>, ClientEntry<&Client>)
    where
        K: ClientKey,
    {
        if self.contains(key) {
            match self.focused {
                // focus the new client and return reference to it and the previously focused client.
                Some(focused) => {
                    self.focused = Some(key.key());
                    (self.get(key), self.get(&focused))
                }
                /*
                not currently focusing any client
                focus the new client and return reference to it.
                */
                None => {
                    self.focused = Some(key.key());
                    (self.get(key), ClientEntry::Vacant)
                }
            }
        } else {
            // key is not a reference to a valid client
            (ClientEntry::Vacant, ClientEntry::Vacant)
        }
    }

    /**
    sets `self.focused` to `None` and returns a reference to the previously focused Client if any.
    */
    pub fn unfocus(&mut self) -> ClientEntry<&Client> {
        match self.focused {
            Some(focused) => {
                self.focused = None;
                self.get(&focused)
            }
            None => ClientEntry::Vacant,
        }
    }

    pub fn is_focused<K>(&self, key: &K) -> bool
    where
        K: ClientKey,
    {
        match self.focused {
            Some(focused) => focused == key.key(),
            None => false,
        }
    }

    pub fn switch_stack_for_client<K>(&mut self, key: &K)
    where
        K: ClientKey,
    {
        if let Some(vs) = self.get_mut_virtualscreen_for_client(key) {
            vs.switch_stack_for_client(key);
        }
    }

    /**
    there should be no need to call this function since a `VirtualScreen` refreshes it self on
    client removal.
    */
    #[deprecated]
    fn refresh_virtual_screen(&mut self) {
        if let Some(vs) = self.virtual_screens.front_mut() {
            vs.refresh();
        }
    }

    /**
    resizes and moves clients on the current virtual screen with `width` and `height` as
    screen width and screen height.
    Optionally adds a gap between windows `gap.unwrap_or(0)` pixels wide.
    */
    pub fn arrange_virtual_screen(&mut self, width: i32, height: i32, gap: Option<i32>) {
        let gap = gap.unwrap_or(0);

        // should be fine to unwrap since we will always have at least 1 virtual screen
        if let Some(vs) = self.virtual_screens.front_mut() {
            // if aux is empty -> width : width / 2
            let width = width / (1 + i32::from(!vs.aux.is_empty()));

            // make sure we dont devide by 0
            let master_height = height
                / match NonZeroI32::new(vs.master.len() as i32) {
                    Some(i) => i.get(),
                    None => 1,
                };

            let aux_height = height
                / match NonZeroI32::new(vs.aux.len() as i32) {
                    Some(i) => i.get(),
                    None => 1,
                };

            // chaining master and aux together with `Zip`s for height and x reduces duplicate code
            for ((i, key), (height, x)) in vs
                .master
                .iter()
                .enumerate()
                // add repeating height for each window and x pos for each window
                .zip(repeat(master_height).zip(repeat(0i32)))
                .chain(
                    vs.aux
                        .iter()
                        .enumerate()
                        .zip(repeat(aux_height).zip(repeat(width))),
                )
            {
                let size = (width - gap * 2, height - gap * 2);
                let position = (x + gap, height * i as i32 + gap);

                if let Some(client) = self.clients.get_mut(key) {
                    *client = Client {
                        size,
                        position,
                        ..*client
                    };
                }
            }
        }

        debug!("{:#?}", self);
    }

    // Should have xlib send those changes back to the x server after this function
}

impl Default for VirtualScreen {
    fn default() -> Self {
        Self {
            master: Default::default(),
            aux: Default::default(),
        }
    }
}

impl VirtualScreen {
    fn contains<K>(&self, key: &K) -> bool
    where
        K: ClientKey,
    {
        self.master.contains(&key.key()) || self.aux.contains(&key.key())
    }

    fn insert<K>(&mut self, key: &K)
    where
        K: ClientKey,
    {
        self.aux.push(key.key());

        self.refresh();
    }

    fn remove<K>(&mut self, key: &K)
    where
        K: ClientKey,
    {
        let key = key.key();
        self.master.retain(|k| *k != key);
        self.aux.retain(|k| *k != key);

        self.refresh();
    }

    fn switch_stack_for_client<K>(&mut self, key: &K)
    where
        K: ClientKey,
    {
        match self.master.iter().position(|&k| k == key.key()) {
            Some(index) => {
                self.aux.extend(self.master.drain(index..=index));
            }
            None => {
                let index = self.aux.iter().position(|&k| k == key.key()).unwrap();
                self.master.extend(self.aux.drain(index..=index));
            }
        }

        self.refresh();
    }

    /**
    if `self.master` is empty but `self.aux` has at least one client, drain from aux to master
    this ensures that if only 1 `Client` is on this `VirtualScreen` it will be on the master stack
    */
    fn refresh(&mut self) {
        if self.master.is_empty() && !self.aux.is_empty() {
            self.master.extend(self.aux.drain(..1));
        }
    }
}
