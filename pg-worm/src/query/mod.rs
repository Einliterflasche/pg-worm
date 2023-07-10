mod filter;
mod join;
mod select;
mod delete;
mod update;
mod table;

use std::ops::Deref;

pub use filter::Filter;
pub use join::{Join, JoinType};
pub use select::*;
pub use delete::*;
pub use update::*;
pub use table::{Column, TypedColumn};

use crate::Model;

#[doc(hidden)]
#[macro_export]
macro_rules! conv_params {
    ($id:expr) => {
        $id.iter()
            .map(|i| &**i as &(dyn ToSql + Sync))
            .collect::<Vec<&(dyn ToSql + Sync)>>()
    };
}

pub fn select<T, const N: usize>(
    cols: [&'static dyn Deref<Target = Column>; N],
) -> SelectBuilder<T> {
    let cols: Vec<_> = cols.into_iter().map(|i| i.deref()).collect();
    SelectBuilder::new(cols.as_slice())
}

pub fn delete<T: Model<T>>() -> DeleteBuilder {
    DeleteBuilder::new(T::table_name())
}

pub fn update<T: Model<T>>() -> UpdateBuilder {
    UpdateBuilder::new(T::table_name())
}
