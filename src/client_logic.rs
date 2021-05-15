#![allow(dead_code)]

use crate::clients2::*;
use std::hash::Hash;

pub struct Size<T> {
    width: T,
    height: T,
}

impl<T> Size<T> {
    pub fn new(width: T, height: T) -> Self {
        Self { width, height }
    }

    /// Get a reference to the size's width.
    pub fn width(&self) -> &T {
        &self.width
    }

    /// Get a reference to the size's height.
    pub fn height(&self) -> &T {
        &self.height
    }

    /// Get a mutable reference to the size's width.
    pub fn width_mut(&mut self) -> &mut T {
        &mut self.width
    }

    /// Get a mutable reference to the size's height.
    pub fn height_mut(&mut self) -> &mut T {
        &mut self.height
    }
}

impl<T> Size<T>
where
    T: Copy,
{
    pub fn dimensions(&self) -> (T, T) {
        (self.width, self.height)
    }
}

pub struct Workspace<T>
where
    T: Eq,
{
    master: Vec<T>,
    aux: Vec<T>,
}

pub struct WorkspaceStore<T>
where
    T: Eq,
{
    workspaces: Vec<Workspace<T>>,
    current_indices: Vec<usize>,
    previous_indices: Option<Vec<usize>>,
}

pub struct ClientManager<T>
where
    T: Hash + Eq + Copy,
{
    store: ClientStore<T>,
    focused: Option<T>,
    workspaces: WorkspaceStore<T>,

    // config
    gap: i32,
    border_size: i32,
    master_size: f32,
    screen_size: Size<i32>,

    // experimental
    invalidated: std::collections::HashSet<T>,
}

impl<T> Default for Workspace<T>
where
    T: Eq,
{
    fn default() -> Self {
        Self {
            master: vec![],
            aux: vec![],
        }
    }
}

impl<T> Default for WorkspaceStore<T>
where
    T: Eq,
{
    fn default() -> Self {
        Self {
            workspaces: vec![Default::default()],
            current_indices: vec![0],
            previous_indices: None,
        }
    }
}

impl<T> Default for ClientManager<T>
where
    T: Hash + Eq + Copy,
{
    fn default() -> Self {
        Self {
            store: Default::default(),
            focused: None,
            workspaces: Default::default(),
            gap: 0,
            border_size: 0,
            master_size: 1.0,
            screen_size: Size::new(1, 1),
            invalidated: Default::default(),
        }
    }
}

enum WorkspaceEntry<T> {
    Master(T),
    Aux(T),
    Vacant,
}

impl<T> Workspace<T>
where
    T: Eq,
{
    pub fn contains(&self, key: &T) -> bool {
        self.aux.contains(key) || self.master.contains(key)
    }

    pub fn is_master(&self, key: &T) -> bool {
        self.master.contains(key)
    }

    pub fn is_aux(&self, key: &T) -> bool {
        self.aux.contains(key)
    }

    pub fn push(&mut self, key: T) {
        self.aux.push(key);
    }

    pub fn remove(&mut self, key: &T) {
        self.master.retain(|k| k != key);
        self.aux.retain(|k| k != key);
    }

    pub fn entry(&self, key: &T) -> Workspace<&T> {
        todo!()
    }
}

impl<T> WorkspaceStore<T>
where
    T: Eq,
{
    pub fn new(num: usize) -> Self {
        let mut workspaces = Vec::with_capacity(num);
        workspaces.resize_with(num, Default::default);

        Self {
            workspaces,
            ..Default::default()
        }
    }

    pub fn remove(&mut self, key: &T) {
        self.iter_mut().for_each(|w| w.remove(key));
    }

    fn len(&self) -> usize {
        self.workspaces.len()
    }

    fn get_current(&self) -> impl Iterator<Item = &Workspace<T>> {
        self.current_indices
            .iter()
            .map(move |&i| &self.workspaces[i])
    }

    fn get_current_mut(
        &mut self,
    ) -> impl Iterator<Item = &mut Workspace<T>> {
        let current_indices = &self.current_indices;

        self.workspaces
            .iter_mut()
            .enumerate()
            .filter(move |(i, _)| current_indices.contains(i))
            .map(|(_, w)| w)
    }

    fn iter_current_master(&self) -> impl Iterator<Item = &T> {
        let current_indices = &self.current_indices;

        self.workspaces
            .iter()
            .enumerate()
            .filter(move |(i, _)| current_indices.contains(i))
            .map(|(_, w)| w)
            .flat_map(|w| w.master.iter())
    }

    fn iter_current_aux(&self) -> impl Iterator<Item = &T> {
        let current_indices = &self.current_indices;

        self.workspaces
            .iter()
            .enumerate()
            .filter(move |(i, _)| current_indices.contains(i))
            .map(|(_, w)| w)
            .flat_map(|w| w.aux.iter())
    }

    fn iter_mut_current_master(
        &mut self,
    ) -> impl Iterator<Item = &mut T> {
        let current_indices = &self.current_indices;

        self.workspaces
            .iter_mut()
            .enumerate()
            .filter(move |(i, _)| current_indices.contains(i))
            .map(|(_, w)| w)
            .flat_map(|w| w.master.iter_mut())
    }

    fn iter_mut_current_aux(
        &mut self,
    ) -> impl Iterator<Item = &mut T> {
        let current_indices = &self.current_indices;

        self.workspaces
            .iter_mut()
            .enumerate()
            .filter(move |(i, _)| current_indices.contains(i))
            .map(|(_, w)| w)
            .flat_map(|w| w.aux.iter_mut())
    }

    fn iter(&self) -> impl Iterator<Item = &Workspace<T>> {
        self.workspaces.iter()
    }

    fn iter_mut(
        &mut self,
    ) -> impl Iterator<Item = &mut Workspace<T>> {
        self.workspaces.iter_mut()
    }

    fn select_workspace(&mut self, idx: usize) {
        let len = self.len();

        self.previous_indices = Some(std::mem::replace(
            &mut self.current_indices,
            vec![idx % len],
        ));
    }

    fn toggle_additional_workspace(&mut self, idx: usize) {
        let idx = idx % self.len();

        if self.current_indices.contains(&idx) {
            self.current_indices.retain(|&i| i != idx);
        } else {
            self.current_indices.push(idx);
        }
    }

    fn select_workspaces<I>(&mut self, idx: I)
    where
        Vec<usize>: From<I>,
    {
        self.previous_indices = Some(std::mem::replace(
            &mut self.current_indices,
            idx.into(),
        ));
    }

    fn select_previous_workspaces(&mut self) {
        if let Some(previous_indices) = &mut self.previous_indices {
            std::mem::swap(
                previous_indices,
                &mut self.current_indices,
            );
        }
    }

    /// Rotate n times left
    fn rotate_left(&mut self, n: usize) {
        let len = self.len();

        let rotate_index = |i| -> usize {
            let a = n % len;
            let b = i & len;

            ((b + len) - a) % len
        };

        self.select_workspaces(
            self.current_indices
                .iter()
                .map(rotate_index)
                .collect::<Vec<_>>(),
        );
    }

    /// Rotate n times left
    fn rotate_right(&mut self, n: usize) {
        let len = self.len();

        let rotate_index = |i| -> usize {
            let a = n % len;
            let b = i & len;

            ((b + len) + a) % len
        };

        self.select_workspaces(
            self.current_indices
                .iter()
                .map(rotate_index)
                .collect::<Vec<_>>(),
        );
    }
}

impl<T> ClientManager<T>
where
    T: Hash + Eq + Copy,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_gap(self, gap: i32) -> Self {
        Self { gap, ..self }
    }

    pub fn with_border(self, border_size: i32) -> Self {
        Self {
            border_size,
            ..self
        }
    }

    pub fn with_screen_size(self, screen_size: Size<i32>) -> Self {
        Self {
            screen_size,
            ..self
        }
    }

    pub fn with_workspaces(self, num: usize) -> Self {
        Self {
            workspaces: WorkspaceStore::new(num),
            ..self
        }
    }

    pub fn new_client(&mut self, client: Client<T>) {
        let entry = self.store.insert(Entry::Tiled(client));

        match entry {
            Entry::Tiled(client) => {
                self.workspaces
                    .get_current_mut()
                    .next()
                    .unwrap()
                    .push(client.window_id());
                self.tile_clients()
            }
            _ => {}
        }
    }

    pub fn remove_client(&mut self, key: &T) {
        if let Some(id) = self.focused {
            if id == *key {
                self.focused = None;
            }
        }

        match self.store.remove(key) {
            // if the window was tiled, remove it from all workspaces and retile all clients
            Entry::Tiled(_) => {
                self.workspaces.remove(key);
                self.tile_clients();
            }
            _ => {}
        };
    }

    pub fn tile_clients(&mut self) {
        let gap = self.gap;
        let border = self.border_size;

        let (width, height) = {
            let dimensions = self.screen_size.dimensions();
            (dimensions.0 - gap * 2, dimensions.1 - gap * 2)
        };

        let len_master =
            self.workspaces.iter_current_master().count();
        let len_aux = self.workspaces.iter_current_aux().count();

        let width_master = match len_aux {
            0 => width,
            _ => width * (self.master_size / 2.0) as i32,
        };
        let width_aux = width - width_master;

        let height_master = match len_master {
            0 | 1 => height,
            n => height / n as i32,
        };
        let height_aux = match len_aux {
            0 | 1 => height,
            n => height / n as i32,
        };

        for (i, id) in
            self.workspaces.iter_mut_current_master().enumerate()
        {
            let size = (
                width_master - gap * 2 - border * 2,
                height_master - gap * 2 - border * 2,
            );

            let position =
                (gap * 2, height_master * i as i32 + gap * 2);

            if let Some(client) =
                Option::<&mut Client<T>>::from(self.store.get_mut(id))
            {
                if *client.position() != position
                    || *client.size() != size
                {
                    *client.position_mut() = position;
                    *client.size_mut() = size;

                    self.invalidated.insert(*id);
                }
            }
        }

        for (i, id) in
            self.workspaces.iter_mut_current_aux().enumerate()
        {
            let size = (
                width_aux - gap * 2 - border * 2,
                height_aux - gap * 2 - border * 2,
            );

            let position = (
                width_master + gap * 2,
                height_aux * i as i32 + gap * 2,
            );

            if let Some(client) =
                Option::<&mut Client<T>>::from(self.store.get_mut(id))
            {
                if *client.position() != position
                    || *client.size() != size
                {
                    *client.position_mut() = position;
                    *client.size_mut() = size;

                    self.invalidated.insert(*id);
                }
            }
        }
    }
}
