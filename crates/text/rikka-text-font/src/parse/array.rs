use std::cmp::Ordering;
use std::fmt;
use std::marker::PhantomData;

pub trait Scalar: Sized + Copy {
    const SIZE: usize;

    fn parse(bytes: &[u8]) -> Option<Self>;
}

#[derive(Clone, Copy)]
pub struct Array<'a, T> {
    data: &'a [u8],
    item: PhantomData<T>,
}

impl<'a, T: Scalar> Array<'a, T> {
    #[inline]
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            item: PhantomData,
        }
    }

    #[inline]
    pub fn empty() -> Self {
        Self::new(&[])
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.data.len() / T::SIZE
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.len() < T::SIZE
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<T> {
        let start = index.checked_mul(T::SIZE)?;
        T::parse(self.data.get(start..start.checked_add(T::SIZE)?)?)
    }

    #[inline]
    pub fn first(&self) -> Option<T> {
        self.get(0)
    }

    #[inline]
    pub fn last(&self) -> Option<T> {
        self.get(self.len().checked_sub(1)?)
    }

    #[inline]
    pub fn iter(&self) -> ArrayIter<'a, T> {
        ArrayIter {
            array: *self,
            index: 0,
        }
    }

    #[inline]
    pub fn binary_search_by(&self, mut compare: impl FnMut(T) -> Ordering) -> Result<usize, usize> {
        let mut low = 0;
        let mut high = self.len();
        while low < high {
            let middle = low + (high - low) / 2;
            let Some(item) = self.get(middle) else {
                return Err(low);
            };
            match compare(item) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Ok(middle),
            }
        }
        Err(low)
    }
}

impl<T: Scalar> Default for Array<'_, T> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<T: Scalar + fmt::Debug> fmt::Debug for Array<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<'a, T: Scalar> IntoIterator for Array<'a, T> {
    type Item = T;
    type IntoIter = ArrayIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Clone, Copy)]
pub struct ArrayIter<'a, T> {
    array: Array<'a, T>,
    index: usize,
}

impl<T: Scalar> Iterator for ArrayIter<'_, T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        let item = self.array.get(self.index)?;
        self.index += 1;
        Some(item)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.array.len().saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

impl<T: Scalar> ExactSizeIterator for ArrayIter<'_, T> {}

macro_rules! be_scalar {
    ($($ty:ty),*) => {$(
        impl Scalar for $ty {
            const SIZE: usize = size_of::<$ty>();

            #[inline]
            fn parse(bytes: &[u8]) -> Option<Self> {
                bytes.first_chunk().map(|b| <$ty>::from_be_bytes(*b))
            }
        }
    )*};
}

be_scalar!(u8, i8, u16, i16, u32, i32);

impl<T: Scalar + fmt::Debug> fmt::Debug for ArrayIter<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArrayIter")
            .field("index", &self.index)
            .finish()
    }
}
