use std::hash::{BuildHasherDefault, Hasher};

#[derive(Debug, Clone, Copy, Default)]
pub struct IdentityHasher(usize);

impl Hasher for IdentityHasher {
    fn finish(&self) -> u64 {
        self.0 as u64
    }

    fn write(&mut self, _bytes: &[u8]) {
        unimplemented!("IdentityHasher only supports usize keys")
    }

    fn write_u64(&mut self, i: u64) {
        self.0 = i as usize;
    }

    fn write_usize(&mut self, i: usize) {
        self.0 = i;
    }
}

pub type BuildIdentityHasher = BuildHasherDefault<IdentityHasher>;
