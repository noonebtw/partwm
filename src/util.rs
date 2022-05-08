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

pub use point::Point;
pub use size::Size;

mod size {
    #[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
    pub struct Size<I>
    where
        I: num_traits::PrimInt + num_traits::Zero,
    {
        pub width: I,
        pub height: I,
    }

    impl<I> std::ops::Add for Size<I>
    where
        I: num_traits::PrimInt + num_traits::Zero,
    {
        type Output = Self;

        fn add(self, rhs: Self) -> Self::Output {
            Self {
                width: self.width + rhs.width,
                height: self.height + rhs.height,
            }
        }
    }

    impl<I> std::ops::Sub for Size<I>
    where
        I: num_traits::PrimInt + num_traits::Zero,
    {
        type Output = Self;

        fn sub(self, rhs: Self) -> Self::Output {
            Self {
                width: self.width - rhs.width,
                height: self.height - rhs.height,
            }
        }
    }

    impl<I> num_traits::Zero for Size<I>
    where
        I: num_traits::PrimInt + num_traits::Zero,
    {
        fn zero() -> Self {
            Self::default()
        }

        fn is_zero(&self) -> bool {
            self.width == I::zero() && self.height == I::zero()
        }
    }

    impl<I> Default for Size<I>
    where
        I: num_traits::PrimInt + num_traits::Zero,
    {
        fn default() -> Self {
            Self {
                width: I::zero(),
                height: I::zero(),
            }
        }
    }

    impl<I> From<(I, I)> for Size<I>
    where
        I: num_traits::PrimInt + num_traits::Zero,
    {
        fn from(value: (I, I)) -> Self {
            Self::from_tuple(value)
        }
    }

    impl<I> From<super::point::Point<I>> for Size<I>
    where
        I: num_traits::PrimInt + num_traits::Zero,
    {
        fn from(value: super::point::Point<I>) -> Self {
            Self::new(value.x, value.y)
        }
    }

    impl<I> Size<I>
    where
        I: num_traits::PrimInt + num_traits::Zero,
    {
        pub fn new(width: I, height: I) -> Self {
            Self { width, height }
        }

        pub fn from_tuple(tuple: (I, I)) -> Self {
            Self::new(tuple.0, tuple.1)
        }

        pub fn as_tuple(&self) -> (I, I) {
            (self.width, self.height)
        }

        pub fn clamp(self, other: Self) -> Self {
            Self::new(
                self.width.min(other.width),
                self.height.min(other.height),
            )
        }

        pub fn map<F>(self, f: F) -> Self
        where
            F: FnOnce(I, I) -> Self,
        {
            f(self.width, self.height)
        }
    }
}

mod point {
    #[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
    pub struct Point<I>
    where
        I: num_traits::PrimInt + num_traits::Zero,
    {
        pub x: I,
        pub y: I,
    }

    impl<I> std::ops::Add for Point<I>
    where
        I: num_traits::PrimInt + num_traits::Zero,
    {
        type Output = Self;

        fn add(self, rhs: Self) -> Self::Output {
            Self {
                x: self.x + rhs.x,
                y: self.y + rhs.y,
            }
        }
    }

    impl<I> std::ops::Sub for Point<I>
    where
        I: num_traits::PrimInt + num_traits::Zero,
    {
        type Output = Self;

        fn sub(self, rhs: Self) -> Self::Output {
            Self {
                x: self.x - rhs.x,
                y: self.y - rhs.y,
            }
        }
    }

    impl<I> num_traits::Zero for Point<I>
    where
        I: num_traits::PrimInt + num_traits::Zero,
    {
        fn zero() -> Self {
            Self::default()
        }

        fn is_zero(&self) -> bool {
            self.x == I::zero() && self.y == I::zero()
        }
    }

    impl<I> Default for Point<I>
    where
        I: num_traits::PrimInt + num_traits::Zero,
    {
        fn default() -> Self {
            Self {
                x: I::zero(),
                y: I::zero(),
            }
        }
    }

    impl<I> From<(I, I)> for Point<I>
    where
        I: num_traits::PrimInt + num_traits::Zero,
    {
        fn from(value: (I, I)) -> Self {
            Self::from_tuple(value)
        }
    }

    impl<I> From<super::size::Size<I>> for Point<I>
    where
        I: num_traits::PrimInt + num_traits::Zero,
    {
        fn from(value: super::size::Size<I>) -> Self {
            Self::new(value.width, value.height)
        }
    }

    impl<I> Point<I>
    where
        I: num_traits::PrimInt + num_traits::Zero,
    {
        pub fn new(x: I, y: I) -> Self {
            Self { x, y }
        }

        pub fn from_tuple(tuple: (I, I)) -> Self {
            Self::new(tuple.0, tuple.1)
        }

        pub fn as_tuple(&self) -> (I, I) {
            (self.x, self.y)
        }

        pub fn map<F, T>(self, f: F) -> T
        where
            F: FnOnce(I, I) -> T,
        {
            f(self.x, self.y)
        }
    }
}
