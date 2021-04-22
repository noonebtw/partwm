use std::{borrow::Borrow, cell::RefCell, collections::HashMap, rc::Rc};
use std::{
    hash::{Hash, Hasher},
    rc::Weak,
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
    fn client_lists_test() {}
}

mod no_refcell {
    use std::{collections::VecDeque, iter::repeat};

    use super::*;

    type ClientsWrapped = Rc<RefCell<Clients>>;
    type Clients = HashMap<Window, Client, BuildIdentityHasher>;
    type ClientRef = u64;
    type ClientRefs = Vec<ClientRef>;

    struct ClientState {
        clients: Clients,
        virtual_screens: VecDeque<VirtualScreen>,
    }

    struct VirtualScreen {
        master: ClientRefs,
        aux: ClientRefs,
        focused: Option<ClientRef>,
    }

    impl ClientState {
        fn insert(&mut self, client: Client) {
            let key = client.key();

            self.clients.insert(key, client);
        }

        fn get<K>(&self, key: &K) -> Option<&Client>
        where
            K: ClientKey,
        {
            self.clients.get(&key.key())
        }

        fn get_mut<K>(&mut self, key: &K) -> Option<&mut Client>
        where
            K: ClientKey,
        {
            self.clients.get_mut(&key.key())
        }

        fn toggle_floating<K>(&mut self, key: &K) -> Option<bool>
        where
            K: ClientKey,
        {
            match self.get_mut(key) {
                Some(client) => {
                    client.floating = !client.floating;
                    Some(client.floating)
                }
                None => None,
            }
        }

        fn get_virtualscreen_for_client<K>(&self, key: &K) -> Option<&VirtualScreen>
        where
            K: ClientKey,
        {
            self.virtual_screens.iter().find_map(
                |vs| {
                    if vs.contains(key) {
                        Some(vs)
                    } else {
                        None
                    }
                },
            )
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

        fn stack_unstacked(&mut self) {
            let unstacked = self
                .clients
                .iter()
                .filter(|&(key, client)| {
                    !client.floating && self.get_virtualscreen_for_client(key).is_some()
                })
                .map(|(key, _)| key)
                .collect::<Vec<_>>();

            match self.virtual_screens.front_mut() {
                Some(vs) => vs.aux.extend(unstacked.into_iter()),
                None => {}
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
                vs.master.retain(|key| match clients.get(key) {
                    Some(client) => !client.floating,
                    None => false,
                });
                vs.aux.retain(|key| match clients.get(key) {
                    Some(client) => !client.floating,
                    None => false,
                });

                // if master is empty but aux has at least one client, drain from aux to master
                if vs.master.is_empty() && !vs.aux.is_empty() {
                    vs.master.extend(vs.aux.drain(..1));
                }
            }
        }

        /**
        resizes and moves clients on the current virtual screen with `width` and `height` as
        screen width and screen height
        */
        fn arange_virtual_screen(&mut self, width: i32, height: i32) {
            // should be fine to unwrap since we will always have at least 1 virtual screen

            if let Some(vs) = self.virtual_screens.front_mut() {
                // if aux is empty -> width : width / 2
                let width = width / 1 + i32::from(!vs.aux.is_empty());

                // chaining master and aux together with `Zip`s for height and x reduces duplicate code
                for ((i, key), (height, x)) in vs
                    .master
                    .iter()
                    .enumerate()
                    // add repeating height for each window and x pos for each window
                    .zip(repeat(height / vs.master.len() as i32).zip(repeat(0i32)))
                    .chain(
                        vs.aux
                            .iter()
                            .enumerate()
                            .zip(repeat(height / vs.aux.len() as i32).zip(repeat(width))),
                    )
                {
                    let size = (width, height);
                    let position = (x, height * i as i32);

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

    impl VirtualScreen {
        fn contains<K>(&self, key: &K) -> bool
        where
            K: ClientKey,
        {
            self.master.contains(&key.key()) || self.aux.contains(&key.key())
        }

        fn focus<K>(&mut self, key: &K)
        where
            K: ClientKey,
        {
            self.focused = Some(key.key());
        }
    }
}

/*
mod refcell {
    use std::collections::VecDeque;

    use super::*;

    type ClientsWrapped = Rc<RefCell<Clients>>;
    type Clients = HashMap<Window, Rc<RefCell<Client>>, BuildIdentityHasher>;
    type ClientRef = Weak<RefCell<Client>>;
    type ClientRefs = Vec<ClientRef>;

    struct ClientState {
        clients: Clients,
        virtual_screens: VecDeque<VirtualScreen>,
    }

    struct VirtualScreen {
        master: ClientRefs,
        aux: ClientRefs,
        focused: Option<ClientRef>,
    }

    impl ClientState {
        fn insert(&mut self, client: Client) {
            let key = client.key();

            self.clients.insert(key, Rc::new(RefCell::new(client)));
        }

        fn get<K>(&self, key: &K) -> Option<&Rc<RefCell<Client>>>
        where
            K: ClientKey,
        {
            self.clients.get(&key.key())
        }

        fn toggle_floating<K>(&mut self, key: &K) -> Option<bool>
        where
            K: ClientKey,
        {
            match self.get(key) {
                Some(client) => {
                    let client = client.borrow_mut();
                    client.floating = !client.floating;

                    Some(client.floating)
                }
                None => None,
            }
        }

        fn get_virtualscreen_for_client<K>(&self, key: &K) -> Option<&VirtualScreen>
        where
            K: ClientKey,
        {
            self.virtual_screens.iter().find_map(
                |vs| {
                    if vs.contains(key) {
                        Some(vs)
                    } else {
                        None
                    }
                },
            )
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

        fn stack_unstacked(&mut self) {
            let unstacked = self
                .clients
                .iter()
                .filter(|&(key, client)| {
                    !client.as_ref().borrow().floating
                        && self.get_virtualscreen_for_client(key).is_some()
                })
                .map(|(key, _)| key)
                .collect::<Vec<_>>();

            match self.virtual_screens.front_mut() {
                Some(vs) => vs.aux.extend(unstacked.into_iter()),
                None => {}
            }
        }

        fn arrange(&mut self) {}
    }

    impl VirtualScreen {
        fn contains<K>(&self, key: &K) -> bool
        where
            K: ClientKey,
        {
            self.master.contains(&key.key()) || self.aux.contains(&key.key())
        }

        fn focus<K>(&mut self, key: &K)
        where
            K: ClientKey,
        {
            self.focused = Some(key.key());
        }
    }
}
*/
