#![allow(dead_code)]
use std::{borrow::Borrow, hash::Hash};

/// Client structure.
#[derive(Clone, Debug)]
pub struct Client<T> {
    window_id: T,
    size: (i32, i32),
    position: (i32, i32),
    transient_for: Option<T>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Entry<T> {
    Tiled(T),
    Floating(T),
    Transient(T),
    Fullscreen(T),
    Vacant,
}

type ClientSet<T> = indexmap::IndexMap<T, Client<T>>;
//type ClientSet<T> = std::collections::HashMap<T, Client<T>>;

pub struct ClientStore<T>
where
    T: Hash + Eq,
{
    tiled_clients: ClientSet<T>,
    floating_clients: ClientSet<T>,
    transient_clients: ClientSet<T>,
    fullscreen_clients: ClientSet<T>,
}

impl<T> PartialEq for Client<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.window_id == other.window_id
    }
}

impl<T> Eq for Client<T> where T: Eq {}

impl<T> PartialOrd for Client<T>
where
    T: PartialOrd,
{
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<std::cmp::Ordering> {
        self.window_id.partial_cmp(&other.window_id)
    }
}

impl<T> Ord for Client<T>
where
    T: Ord,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.window_id.cmp(&other.window_id)
    }
}

impl<T> Hash for Client<T>
where
    T: Hash,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.window_id.hash(state)
    }
}

impl<T> Borrow<T> for Client<T> {
    fn borrow(&self) -> &T {
        &self.window_id
    }
}

impl<T> Client<T> {
    pub fn new(window_id: T) -> Self {
        Self {
            window_id,
            size: (100, 100),
            position: (0, 0),
            transient_for: None,
        }
    }

    pub fn window_id_ref(&self) -> &T {
        &self.window_id
    }

    /// Get a mutable reference to the client's size.
    pub fn size_mut(&mut self) -> &mut (i32, i32) {
        &mut self.size
    }

    /// Get a mutable reference to the client's position.
    pub fn position_mut(&mut self) -> &mut (i32, i32) {
        &mut self.position
    }

    /// Get a reference to the client's size.
    pub fn size(&self) -> &(i32, i32) {
        &self.size
    }

    /// Get a reference to the client's position.
    pub fn position(&self) -> &(i32, i32) {
        &self.position
    }
}

impl<T> Client<T>
where
    T: Copy,
{
    pub fn window_id(&self) -> T {
        self.window_id
    }
}

impl<T> From<Entry<T>> for Option<T> {
    fn from(entry: Entry<T>) -> Self {
        match entry {
            Entry::Floating(c)
            | Entry::Tiled(c)
            | Entry::Fullscreen(c)
            | Entry::Transient(c) => Some(c),
            _ => None,
        }
    }
}

impl<'a, T> From<&'a Entry<T>> for Option<&'a T> {
    fn from(entry: &'a Entry<T>) -> Self {
        match entry {
            Entry::Floating(c)
            | Entry::Tiled(c)
            | Entry::Fullscreen(c)
            | Entry::Transient(c) => Some(c),
            _ => None,
        }
    }
}

impl<'a, T> From<&'a mut Entry<T>> for Option<&'a mut T> {
    fn from(entry: &'a mut Entry<T>) -> Self {
        match entry {
            Entry::Floating(c)
            | Entry::Tiled(c)
            | Entry::Fullscreen(c)
            | Entry::Transient(c) => Some(c),
            _ => None,
        }
    }
}

impl<T> From<Option<Entry<T>>> for Entry<T> {
    fn from(opt: Option<Entry<T>>) -> Self {
        match opt {
            Some(entry) => entry,
            None => Entry::Vacant,
        }
    }
}

impl<T> Entry<T> {
    pub fn unwrap(self) -> T {
        Option::<T>::from(self).unwrap()
    }

    pub fn unwrap_ref(&self) -> &T {
        Option::<&T>::from(self).unwrap()
    }

    pub fn unwrap_mut(&mut self) -> &mut T {
        Option::<&mut T>::from(self).unwrap()
    }

    pub fn is_floating(&self) -> bool {
        match self {
            Self::Floating(_) => true,
            _ => false,
        }
    }

    pub fn is_tiled(&self) -> bool {
        match self {
            Self::Tiled(_) => true,
            _ => false,
        }
    }

    pub fn is_transient(&self) -> bool {
        match self {
            Self::Transient(_) => true,
            _ => false,
        }
    }

    pub fn is_fullscreen(&self) -> bool {
        match self {
            Self::Fullscreen(_) => true,
            _ => false,
        }
    }
}

impl<T> Default for ClientStore<T>
where
    T: Hash + Eq,
{
    fn default() -> Self {
        Self {
            tiled_clients: Default::default(),
            floating_clients: Default::default(),
            transient_clients: Default::default(),
            fullscreen_clients: Default::default(),
        }
    }
}

impl<T> ClientStore<T>
where
    T: Hash + Eq + Copy,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        entry: Entry<Client<T>>,
    ) -> Entry<&Client<T>> {
        if let Some(key) =
            Option::<&Client<T>>::from(&entry).map(|c| c.window_id())
        {
            match entry {
                Entry::Floating(client) => {
                    self.floating_clients.insert(key, client);
                }
                Entry::Tiled(client) => {
                    self.tiled_clients.insert(key, client);
                }
                Entry::Transient(client) => {
                    self.transient_clients.insert(key, client);
                }
                Entry::Fullscreen(client) => {
                    self.fullscreen_clients.insert(key, client);
                }
                _ => unreachable!(),
            }

            self.get(&key).into()
        } else {
            Entry::Vacant
        }
    }

    pub fn remove(&mut self, key: &T) -> Entry<Client<T>> {
        if let Some(client) = self.tiled_clients.remove(key) {
            Entry::Tiled(client)
        } else if let Some(client) = self.floating_clients.remove(key)
        {
            Entry::Floating(client)
        } else if let Some(client) =
            self.transient_clients.remove(key)
        {
            Entry::Transient(client)
        } else if let Some(client) =
            self.fullscreen_clients.remove(key)
        {
            Entry::Fullscreen(client)
        } else {
            Entry::Vacant
        }
    }

    pub fn get(&self, key: &T) -> Entry<&Client<T>> {
        if let Some(client) = self.tiled_clients.get(key) {
            Entry::Tiled(client)
        } else if let Some(client) = self.floating_clients.get(key) {
            Entry::Floating(client)
        } else if let Some(client) = self.transient_clients.get(key) {
            Entry::Transient(client)
        } else if let Some(client) = self.fullscreen_clients.get(key)
        {
            Entry::Fullscreen(client)
        } else {
            Entry::Vacant
        }
    }

    pub fn get_mut(&mut self, key: &T) -> Entry<&mut Client<T>> {
        if let Some(client) = self.tiled_clients.get_mut(key) {
            Entry::Tiled(client)
        } else if let Some(client) =
            self.floating_clients.get_mut(key)
        {
            Entry::Floating(client)
        } else if let Some(client) =
            self.transient_clients.get_mut(key)
        {
            Entry::Transient(client)
        } else if let Some(client) =
            self.fullscreen_clients.get_mut(key)
        {
            Entry::Fullscreen(client)
        } else {
            Entry::Vacant
        }
    }

    pub fn contains(&self, key: &T) -> bool {
        self.tiled_clients.contains_key(key)
            || self.floating_clients.contains_key(key)
            || self.transient_clients.contains_key(key)
            || self.fullscreen_clients.contains_key(key)
    }

    pub fn iter_tiled(
        &self,
    ) -> impl Iterator<Item = (&T, &Client<T>)> {
        self.tiled_clients.iter()
    }

    pub fn iter_mut_tiled(
        &mut self,
    ) -> impl Iterator<Item = (&T, &mut Client<T>)> {
        self.tiled_clients.iter_mut()
    }

    pub fn iter_floating(
        &self,
    ) -> impl Iterator<Item = (&T, &Client<T>)> {
        self.floating_clients.iter()
    }

    pub fn iter_mut_floating(
        &mut self,
    ) -> impl Iterator<Item = (&T, &mut Client<T>)> {
        self.floating_clients.iter_mut()
    }

    pub fn iter_transient(
        &self,
    ) -> impl Iterator<Item = (&T, &Client<T>)> {
        self.transient_clients.iter()
    }

    pub fn iter_mut_transient(
        &mut self,
    ) -> impl Iterator<Item = (&T, &mut Client<T>)> {
        self.transient_clients.iter_mut()
    }

    pub fn iter_fullscreen(
        &self,
    ) -> impl Iterator<Item = (&T, &Client<T>)> {
        self.fullscreen_clients.iter()
    }

    pub fn iter_mut_fullscreen(
        &mut self,
    ) -> impl Iterator<Item = (&T, &mut Client<T>)> {
        self.fullscreen_clients.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clientstore_insert_contain() {
        let client = Client::new(1u64);

        let mut client_store = ClientStore::new();
        client_store.insert(Entry::Tiled(client.clone()));

        assert!(client_store.contains(client.borrow()));
        assert!(client_store.contains(&1));

        let client2 = Client::new(3u64);
        client_store.insert(Entry::Floating(client2.clone()));

        assert!(client_store.contains(&client.borrow()));
        assert!(client_store.contains(&1));

        assert!(client_store.contains(&client2.borrow()));
        assert!(client_store.contains(&3));

        assert_eq!(
            Entry::Tiled(client.clone()),
            client_store.remove(&client.borrow())
        );
        assert_eq!(
            Entry::Vacant,
            client_store.remove(&client.borrow())
        );
        assert_eq!(Entry::Vacant, client_store.remove(&1));

        assert!(client_store.contains(&client2.borrow()));
    }
}
