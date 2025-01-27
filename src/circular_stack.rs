use core::{mem::MaybeUninit, ops, ptr, slice};

/// Fixed capacity Stack which goes around when doesn't have enough space,
/// replacing lowest item in stack.
///  
/// Optimizations are taken from heapless::Vec,
/// because underlying structure is same, just array.
pub struct CircularStack<T, const N: usize> {
    // Order is important for optimizations
    top_index: usize,
    len: usize,

    buffer: [MaybeUninit<T>; N],
}

impl<T, const N: usize> CircularStack<T, N> {
    // Optimizations stuff
    const ELEM: MaybeUninit<T> = MaybeUninit::uninit();
    const INIT: [MaybeUninit<T>; N] = [Self::ELEM; N];

    pub const fn new() -> Self {
        Self {
            top_index: 0,
            len: 0,
            buffer: Self::INIT,
        }
    }

    pub fn push(&mut self, item: T) {
        self.top_index += 1;
        if self.top_index == self.capacity() {
            self.top_index = 0;
        }

        // I already perform bounds check above
        unsafe {
            *self.buffer.get_unchecked_mut(self.top_index) = MaybeUninit::new(item);
        }

        if self.len < self.capacity() {
            self.len += 1;
        }
    }
   pub  fn remove(&mut self, index: usize) {

    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub const fn capacity(&self) -> usize {
        N
    }

 


    // Copied from heapless::Vec
    pub fn as_slice(&self) -> &[T] {
        // NOTE(unsafe) avoid bound checks in the slicing operation
        // &buffer[..self.len]
        unsafe { slice::from_raw_parts(self.buffer.as_ptr() as *const T, self.len) }
    }
    fn as_mut_slice(&mut self) -> &mut [T] {
        // NOTE(unsafe) avoid bound checks in the slicing operation
        // &mut buffer[..self.len]
        unsafe { slice::from_raw_parts_mut(self.buffer.as_mut_ptr() as *mut T, self.len) }
    }
}

// Traits
impl<T, const N: usize> ops::Deref for CircularStack<T, N> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, const N: usize> Drop for CircularStack<T, N> {
    fn drop(&mut self) {
        // We drop each element used in the vector by turning into a &mut[T]
        unsafe {
            ptr::drop_in_place(self.as_mut_slice());
        }
    }
}
