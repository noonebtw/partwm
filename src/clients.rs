use std::{borrow::Borrow, cell::RefCell, collections::HashMap, rc::Rc};
use std::{
    hash::{Hash, Hasher},
    num::NonZeroI32,
};

use x11::xlib::Window;

use crate::util::BuildIdentityHasher;

#[derive(Clone, Debug)]
struct Client {
    window: Window,
    floating: bool,
    size: (i32, i32),
    position: (i32, i32),
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

impl Borrow<Window> for Client {
    fn borrow(&self) -> &Window {
        &self.window
    }
}

trait ClientKey {
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

impl ClientKey for Rc<Client> {
    fn key(&self) -> u64 {
        self.window
    }
}

impl ClientKey for Window {
    fn key(&self) -> u64 {
        self.to_owned()
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
            floating: false,
        });

        clients.insert(Client {
            window: 2,
            size: (1, 1),
            position: (1, 1),
            floating: false,
        });

        clients.refresh_virtual_screen();
        clients.arange_virtual_screen(600, 400);

        println!("{:#?}", clients);

        clients.remove(&1u64);

        clients.refresh_virtual_screen();
        clients.arange_virtual_screen(600, 400);

        println!("{:#?}", clients);

        clients.virtual_screens.rotate_right(1);

        clients.insert(Client {
            window: 3,
            size: (1, 1),
            position: (1, 1),
            floating: false,
        });

        clients.refresh_virtual_screen();
        clients.arange_virtual_screen(600, 400);

        println!("{:#?}", clients);

        clients.toggle_floating(&2u64);

        clients.virtual_screens.rotate_left(1);

        clients.stack_unstacked();
        clients.refresh_virtual_screen();
        clients.arange_virtual_screen(600, 400);

        println!("{:#?}", clients);
    }
}

use std::{collections::VecDeque, iter::repeat};

type Clients = HashMap<Window, Client, BuildIdentityHasher>;
type ClientRef = u64;
type ClientRefs = Vec<ClientRef>;

#[derive(Debug, Clone)]
struct ClientState {
    clients: Clients,
    floating_clients: Clients,
    virtual_screens: VecDeque<VirtualScreen>,
}

#[derive(Debug, Clone)]
struct VirtualScreen {
    master: ClientRefs,
    aux: ClientRefs,
    focused: Option<ClientRef>,
}

impl Default for ClientState {
    fn default() -> Self {
        let mut vss = VecDeque::<VirtualScreen>::new();
        vss.resize_with(10, Default::default);

        Self {
            clients: Default::default(),
            floating_clients: Default::default(),
            virtual_screens: vss,
        }
    }
}

impl ClientState {
    fn new() -> Self {
        Self::default()
    }

    fn with_virtualscreens(num: usize) -> Self {
        let mut vss = VecDeque::<VirtualScreen>::new();
        vss.resize_with(num, Default::default);

        Self {
            virtual_screens: vss,
            ..Default::default()
        }
    }

    fn insert(&mut self, client: Client) {
        let key = client.key();

        self.clients.insert(key, client);

        if let Some(vs) = self.virtual_screens.front_mut() {
            vs.aux.push(key);
        }
    }

    fn remove<K>(&mut self, key: &K)
    where
        K: ClientKey,
    {
        self.remove_from_virtual_screens(key);

        self.clients.remove(&key.key());
        self.floating_clients.remove(&key.key());
    }

    fn get<K>(&self, key: &K) -> Option<&Client>
    where
        K: ClientKey,
    {
        self.clients
            .get(&key.key())
            .or_else(|| self.floating_clients.get(&key.key()))
    }

    fn get_mut<K>(&mut self, key: &K) -> Option<&mut Client>
    where
        K: ClientKey,
    {
        match self.clients.get_mut(&key.key()) {
            Some(client) => Some(client),
            None => self.floating_clients.get_mut(&key.key()),
        }
    }

    fn toggle_floating<K>(&mut self, key: &K)
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
                    vs.aux.push(key);
                }
            }
            _ => {}
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

    /// focuses client `key` on current virtual screen
    fn focus_client<K>(&mut self, key: &K)
    where
        K: ClientKey,
    {
        match self.virtual_screens.front_mut() {
            Some(vs) => vs.focus(key),
            None => {}
        }
    }

    /**
    This shouldnt be ever needed to be called since any client added is automatically added
    to the first `VirtualScreen`
    */
    fn stack_unstacked(&mut self) {
        let unstacked = self
            .clients
            .iter()
            .filter(|&(key, _)| self.get_virtualscreen_for_client(key).is_none())
            .map(|(key, _)| key)
            .collect::<Vec<_>>();

        if let Some(vs) = self.virtual_screens.front_mut() {
            vs.aux.extend(unstacked.into_iter());
        }
    }

    fn switch_stack_for_client<K>(&mut self, key: &K)
    where
        K: ClientKey,
    {
        if let Some(vs) = self.get_mut_virtualscreen_for_client(key) {
            match vs.master.iter().position(|&key| key == key.key()) {
                Some(index) => {
                    vs.aux.extend(vs.master.drain(index..=index));
                }
                None => {
                    let index = vs.aux.iter().position(|&key| key == key.key()).unwrap();
                    vs.master.extend(vs.aux.drain(index..=index));
                }
            }
        }
    }

    fn refresh_virtual_screen(&mut self) {
        let clients = &self.clients;

        if let Some(vs) = self.virtual_screens.front_mut() {
            vs.refresh();
        }
    }

    /**
    resizes and moves clients on the current virtual screen with `width` and `height` as
    screen width and screen height
    */
    fn arange_virtual_screen(&mut self, width: i32, height: i32, gap: Option<i32>) {
        let gap = gap.unwrap_or(0);

        // should be fine to unwrap since we will always have at least 1 virtual screen
        if let Some(vs) = self.virtual_screens.front_mut() {
            // if aux is empty -> width : width / 2
            let width = width / (1 + i32::from(!vs.aux.is_empty() && !vs.master.is_empty()));

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
                let size = (width + gap * 2, height + gap * 2);
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
    }

    // Should have xlib send those changes back to the x server after this function
}

impl Default for VirtualScreen {
    fn default() -> Self {
        Self {
            master: Default::default(),
            aux: Default::default(),
            focused: None,
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

    fn remove<K>(&mut self, key: &K)
    where
        K: ClientKey,
    {
        let key = key.key();
        self.master.retain(|k| *k != key);
        self.aux.retain(|k| *k != key);

        if let Some(k) = self.focused {
            if k == key {
                self.focused = None;
            }
        }
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

    fn focus<K>(&mut self, key: &K)
    where
        K: ClientKey,
    {
        self.focused = Some(key.key());
    }
}
