use std::ops::Range;
use std::path::PathBuf;

use crate::{constraint, scheme};

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("Inconsistent prefix part lengths")]
    InconsistentPrefixPartLengths {
        prefix_part_lengths_total: usize,
        length_constraint: constraint::Length,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeBuilder<S> {
    base: PathBuf,
    length_constraint: Option<crate::constraint::Length>,
    extension_constraint: Option<crate::constraint::Extension>,
    prefix_part_lengths: Option<Vec<usize>>,
    scheme: S,
}

impl TreeBuilder<crate::scheme::Identity> {
    pub(crate) const fn new(base: PathBuf) -> Self {
        Self {
            base,
            length_constraint: None,
            extension_constraint: None,
            prefix_part_lengths: None,
            scheme: scheme::Identity,
        }
    }
}

impl<S> TreeBuilder<S> {
    pub fn build(self) -> Result<crate::Tree<S>, Error> {
        let tree = self.into_tree();

        match tree.length_constraint {
            Some(length_constraint) => {
                let prefix_part_lengths_total =
                    tree.prefix_part_lengths.iter().copied().sum::<usize>();

                match length_constraint {
                    constraint::Length::Fixed(length) | constraint::Length::Range(_, length) => {
                        if prefix_part_lengths_total <= length {
                            Ok(tree)
                        } else {
                            Err(Error::InconsistentPrefixPartLengths {
                                prefix_part_lengths_total,
                                length_constraint,
                            })
                        }
                    }
                }
            }
            None => Ok(tree),
        }
    }

    /// Internal unvalidated conversion.
    fn into_tree(self) -> crate::Tree<S> {
        crate::Tree {
            base: self.base,
            length_constraint: self.length_constraint,
            extension_constraint: self.extension_constraint,
            prefix_part_lengths: self.prefix_part_lengths.unwrap_or_default(),
            scheme: self.scheme,
        }
    }

    #[must_use]
    pub fn with_no_extension(self) -> Self {
        Self {
            base: self.base,
            length_constraint: self.length_constraint,
            extension_constraint: Some(crate::constraint::Extension::None),
            prefix_part_lengths: self.prefix_part_lengths,
            scheme: self.scheme,
        }
    }

    #[must_use]
    pub fn with_extension<E: Into<String>>(self, extension: E) -> Self {
        Self {
            base: self.base,
            length_constraint: self.length_constraint,
            extension_constraint: Some(crate::constraint::Extension::Fixed(extension.into())),
            prefix_part_lengths: self.prefix_part_lengths,
            scheme: self.scheme,
        }
    }

    #[must_use]
    pub fn with_any_extension(self) -> Self {
        Self {
            base: self.base,
            length_constraint: self.length_constraint,
            extension_constraint: Some(crate::constraint::Extension::Any),
            prefix_part_lengths: self.prefix_part_lengths,
            scheme: self.scheme,
        }
    }

    #[must_use]
    pub fn with_length(self, length: usize) -> Self {
        Self {
            base: self.base,
            length_constraint: Some(length.into()),
            extension_constraint: self.extension_constraint,
            prefix_part_lengths: self.prefix_part_lengths,
            scheme: self.scheme,
        }
    }

    #[must_use]
    pub fn with_length_range(self, range: Range<usize>) -> Self {
        Self {
            base: self.base,
            length_constraint: Some(range.into()),
            extension_constraint: self.extension_constraint,
            prefix_part_lengths: self.prefix_part_lengths,
            scheme: self.scheme,
        }
    }

    #[must_use]
    pub fn with_prefix_part_lengths<T: AsRef<[usize]>>(self, prefix_part_lengths: T) -> Self {
        Self {
            base: self.base,
            length_constraint: self.length_constraint,
            extension_constraint: self.extension_constraint,
            prefix_part_lengths: Some(prefix_part_lengths.as_ref().to_vec()),
            scheme: self.scheme,
        }
    }

    #[must_use]
    pub fn with_scheme<T: crate::scheme::Scheme>(self, scheme: T) -> TreeBuilder<T> {
        let length_constraint = T::fixed_length().map_or(self.length_constraint, |fixed_length| {
            Some(fixed_length.into())
        });

        TreeBuilder {
            base: self.base,
            length_constraint,
            extension_constraint: self.extension_constraint,
            prefix_part_lengths: self.prefix_part_lengths,
            scheme,
        }
    }
}
