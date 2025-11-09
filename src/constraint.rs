use std::ops::Range;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Extension {
    #[default]
    None,
    Any,
    Fixed(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Length {
    Fixed(usize),
    Range(usize, usize),
}

impl From<usize> for Length {
    fn from(value: usize) -> Self {
        Self::Fixed(value)
    }
}

impl From<Range<usize>> for Length {
    fn from(value: Range<usize>) -> Self {
        Self::Range(value.start, value.end)
    }
}
