//! ```
//! #[derive(Entity)]
//! struct Person {
//!     id: i64,
//!     name: String
//! }
pub use pg_worm_derive::Entity;
use std::marker::PhantomData;

use phf::Map;
use tokio_postgres::{{types::Type}, Row};

pub struct Table<T> {
    name: &'static str,
    fields: &'static Map<&'static str, Type>,
    type_: PhantomData<T>
}

pub trait Entity<T> {
    fn from_sql(row: &Row) -> Result<T, tokio_postgres::Error>;
}
