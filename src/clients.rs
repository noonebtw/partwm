use std::{
    borrow::{Borrow, BorrowMut},
    collections::HashSet,
    ops::{Deref, DerefMut},
    rc::Rc,
};
use std::{
    hash::{Hash, Hasher},
    rc::Weak,
};

use weak_table::WeakHashSet;
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

trait ClientList {
    fn contains_key<T>(&self, key: &T) -> bool
    where
        T: ClientKey;

    fn get_with_key<T>(&self, key: &T) -> Option<Rc<Client>>
    where
        T: ClientKey;

    fn remove_key<T>(&mut self, key: &T) -> bool
    where
        T: ClientKey;
}

struct Clients(HashSet<Rc<Client>, BuildIdentityHasher>);

impl Default for Clients {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl Deref for Clients {
    type Target = HashSet<Rc<Client>, BuildIdentityHasher>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Clients {
    fn deref_mut(&mut self) -> &mut HashSet<Rc<Client>, BuildIdentityHasher> {
        &mut self.0
    }
}

impl ClientList for Clients {
    fn contains_key<T>(&self, key: &T) -> bool
    where
        T: ClientKey,
    {
        self.0.contains(key as &dyn ClientKey)
    }

    fn get_with_key<T>(&self, key: &T) -> Option<Rc<Client>>
    where
        T: ClientKey,
    {
        self.0.get(key as &dyn ClientKey).cloned()
    }

    fn remove_key<T>(&mut self, key: &T) -> bool
    where
        T: ClientKey,
    {
        self.0.remove(key as &dyn ClientKey)
    }
}

struct ClientRefs(WeakHashSet<Weak<Client>, BuildIdentityHasher>);

impl Default for ClientRefs {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl Deref for ClientRefs {
    type Target = WeakHashSet<Weak<Client>, BuildIdentityHasher>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ClientRefs {
    fn deref_mut(&mut self) -> &mut WeakHashSet<Weak<Client>, BuildIdentityHasher> {
        &mut self.0
    }
}

impl ClientList for ClientRefs {
    fn contains_key<T>(&self, key: &T) -> bool
    where
        T: ClientKey,
    {
        self.0.contains(key as &dyn ClientKey)
    }

    fn get_with_key<T>(&self, key: &T) -> Option<Rc<Client>>
    where
        T: ClientKey,
    {
        self.0.get(key as &dyn ClientKey)
    }

    fn remove_key<T>(&mut self, key: &T) -> bool
    where
        T: ClientKey,
    {
        self.0.remove(key as &dyn ClientKey)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_lists_test() {
        let mut clients: Clients = Default::default();

        clients.insert(Rc::new(Client {
            window: 1,
            floating: false,
            position: (1, 1),
            size: (1, 1),
        }));

        assert!(clients.contains_key(&1u64));

        let mut client_refs = ClientRefs::default();

        client_refs.insert(clients.get_with_key(&1u64).unwrap());

        assert!(client_refs.contains_key(&1u64));

        clients.remove_key(&1u64);

        assert!(!client_refs.contains_key(&1u64));
    }
}
