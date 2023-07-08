mod filter;
mod join;
mod select;
mod table;

use std::ops::Deref;

pub use filter::Filter;
pub use join::{Join, JoinType};
pub use select::*;
pub use table::{Column, TypedColumn};

/// An executable query. Built using a [`QueryBuilder`].
pub struct Query;

impl Query {
    pub fn select<T, const N: usize>(
        cols: [&'static dyn Deref<Target = Column>; N],
    ) -> SelectBuilder<T> {
        let cols: Vec<_> = cols.into_iter().map(|i| i.deref()).collect();
        SelectBuilder::new(cols.as_slice())
    }
}
