use arrow_array::Array;

pub trait BoxedArray {
    fn boxed(self) -> Box<dyn Array>;
}

impl<T> BoxedArray for T
where
    T: Array + 'static,
{
    fn boxed(self) -> Box<dyn Array> {
        Box::new(self)
    }
}

#[derive(Debug, Clone)]
pub struct Chunk<T> {
    arrays: Vec<T>,
    len: usize,
}

impl<T> Chunk<T>
where
    T: AsRef<dyn Array>,
{
    pub fn new(arrays: Vec<T>) -> Self {
        let len = arrays
            .first()
            .map(|array| array.as_ref().len())
            .unwrap_or(0);
        for array in arrays.iter().skip(1) {
            assert_eq!(
                array.as_ref().len(),
                len,
                "all chunk arrays must have equal length"
            );
        }
        Self { arrays, len }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn arrays(&self) -> &[T] {
        &self.arrays
    }

    pub fn into_arrays(self) -> Vec<T> {
        self.arrays
    }
}
