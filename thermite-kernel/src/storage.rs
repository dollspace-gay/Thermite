#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageError {
    OutOfBounds,
    Full,
    Empty,
    DuplicateKey,
    MissingKey,
}

/// No-allocation fixed-capacity storage used before the verified heap is live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedVec<T: Copy + Default, const N: usize> {
    values: [T; N],
    len: usize,
}

impl<T: Copy + Default, const N: usize> Default for FixedVec<T, N> {
    fn default() -> Self {
        Self {
            values: [T::default(); N],
            len: 0,
        }
    }
}

impl<T: Copy + Default, const N: usize> FixedVec<T, N> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, index: usize) -> Result<T, StorageError> {
        if index >= self.len {
            return Err(StorageError::OutOfBounds);
        }
        Ok(self.values[index])
    }

    pub fn set(&mut self, index: usize, value: T) -> Result<(), StorageError> {
        if index >= self.len {
            return Err(StorageError::OutOfBounds);
        }
        self.values[index] = value;
        Ok(())
    }

    pub fn push(&mut self, value: T) -> Result<(), StorageError> {
        if self.len == N {
            return Err(StorageError::Full);
        }
        self.values[self.len] = value;
        self.len += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Result<T, StorageError> {
        if self.len == 0 {
            return Err(StorageError::Empty);
        }
        self.len -= 1;
        Ok(self.values[self.len])
    }

    pub fn as_slice(&self) -> &[T] {
        &self.values[..self.len]
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.values[..self.len]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedMap<K: Copy + Eq, V: Copy, const N: usize> {
    entries: [Option<(K, V)>; N],
    len: usize,
}

impl<K: Copy + Eq, V: Copy, const N: usize> Default for FixedMap<K, V, N> {
    fn default() -> Self {
        Self {
            entries: [None; N],
            len: 0,
        }
    }
}

impl<K: Copy + Eq, V: Copy, const N: usize> FixedMap<K, V, N> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: K, value: V) -> Result<(), StorageError> {
        if self
            .entries
            .iter()
            .flatten()
            .any(|(stored, _)| *stored == key)
        {
            return Err(StorageError::DuplicateKey);
        }
        if self.len == N {
            return Err(StorageError::Full);
        }
        self.entries[self.len] = Some((key, value));
        self.len += 1;
        Ok(())
    }

    pub fn get(&self, key: K) -> Result<V, StorageError> {
        self.entries
            .iter()
            .flatten()
            .find_map(|(stored, value)| (*stored == key).then_some(*value))
            .ok_or(StorageError::MissingKey)
    }

    pub fn remove(&mut self, key: K) -> Result<V, StorageError> {
        let Some(index) = self
            .entries
            .iter()
            .take(self.len)
            .position(|entry| matches!(entry, Some((stored, _)) if *stored == key))
        else {
            return Err(StorageError::MissingKey);
        };
        let Some((_, value)) = self.entries[index] else {
            return Err(StorageError::MissingKey);
        };
        let last = self.len - 1;
        self.entries[index] = self.entries[last];
        self.entries[last] = None;
        self.len = last;
        Ok(value)
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap<const WORDS: usize> {
    words: [u64; WORDS],
}

impl<const WORDS: usize> Default for Bitmap<WORDS> {
    fn default() -> Self {
        Self { words: [0; WORDS] }
    }
}

impl<const WORDS: usize> Bitmap<WORDS> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, bit: usize, value: bool) -> Result<(), StorageError> {
        let word = bit / 64;
        if word >= WORDS {
            return Err(StorageError::OutOfBounds);
        }
        let mask = 1u64 << (bit % 64);
        if value {
            self.words[word] |= mask;
        } else {
            self.words[word] &= !mask;
        }
        Ok(())
    }

    pub fn get(&self, bit: usize) -> Result<bool, StorageError> {
        let word = bit / 64;
        if word >= WORDS {
            return Err(StorageError::OutOfBounds);
        }
        Ok(self.words[word] & (1u64 << (bit % 64)) != 0)
    }

    #[must_use]
    pub fn first_clear(&self) -> Option<usize> {
        self.words.iter().enumerate().find_map(|(index, word)| {
            if *word == u64::MAX {
                None
            } else {
                Some(index * 64 + (!word).trailing_zeros() as usize)
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RingBuffer<T: Copy + Default, const N: usize> {
    values: [T; N],
    head: usize,
    len: usize,
}

impl<T: Copy + Default, const N: usize> Default for RingBuffer<T, N> {
    fn default() -> Self {
        Self {
            values: [T::default(); N],
            head: 0,
            len: 0,
        }
    }
}

impl<T: Copy + Default, const N: usize> RingBuffer<T, N> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, value: T) -> Result<(), StorageError> {
        if self.len == N || N == 0 {
            return Err(StorageError::Full);
        }
        let tail = (self.head + self.len) % N;
        self.values[tail] = value;
        self.len += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Result<T, StorageError> {
        if self.len == 0 || N == 0 {
            return Err(StorageError::Empty);
        }
        let value = self.values[self.head];
        self.head = (self.head + 1) % N;
        self.len -= 1;
        Ok(value)
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}
