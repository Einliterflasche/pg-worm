//! The goal is to create a simple, easy-to-use ORM for PostgreSQL databases.
//! 
//! Currently only offers a derivable `Model` trait for parsing structs from `tokio_postgres::Row`.
//! 
//! # Usage
//! 
//! ```
//! use pg_worm::Model;
//! use tokio_postgres::{NoTls, connect};
//! 
//! #[derive(Debug, Model)]
//! struct Person {
//!     id: i64,
//!     name: String
//! }
//! 
//! #[tokio::main]
//! async fn main() {
//!     let (client, conn) = connect("postgres://me:me@localhost:5432", NoTls).await.unwrap();
//!     tokio::spawn(async move { conn.await.unwrap() } );
//! 
//!     client.execute(
//!         "CREATE TABLE IF NOT EXISTS person (
//!             id BIGSERIAL PRIMARY KEY UNIQUE,
//!             name TEXT
//!         )",
//!         &[]
//!     ).await.unwrap();
//! 
//!     client.execute("INSERT INTO person (name) VALUES ($1)", &[&"Jesus"]).await.unwrap();
//! 
//!     let rows = client.query("SELECT id, name FROM person", &[]).await.unwrap();
//! 
//!     let person = Person::from_row(rows.first().unwrap()).unwrap();
//!     assert_eq!(person.name, "Jesus");
//!     assert_eq!(person.id, 1)
//! }
//! ```

use tokio_postgres::Row;

pub use pg_worm_derive::Model;

/// This trait allows comfortable querying.
///
/// # Usage
///
/// ```
/// use pg_worm::Model;
///
/// #[derive(Model)]
/// struct Book {
///     id: i64,
///     title: String
/// }
/// ```
pub trait Model<T> {
    ///
    fn from_row(row: &Row) -> Result<T, tokio_postgres::Error>;
}

#[cfg(test)]
mod tests {
    use crate::Model;

    #[derive(Model)]
    struct Person {
        id: i64,
        name: String,
    }
}
