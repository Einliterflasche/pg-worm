//! The goal is to create a simple, easy-to-use ORM for PostgreSQL databases.
//!
//! Currently only offers a derivable `Model` trait for parsing structs from `tokio_postgres::Row`.
//!
//! # Usage
//!
//! ```
//! use pg_worm::tokio_postgres::NoTls;
//! use pg_worm::{connect, Model, get_client};
//! 
//! #[derive(Model)]
//! struct Book {
//!     #[column(dtype = "BIGSERIAL", primary_key, unique)]
//!     id: i64,
//!     #[column(dtype = "TEXT", unique)]
//!     title: String,
//! }
//! 
//! #[tokio::main]
//! async fn main() {
//!     let conn = connect("postgres://me:me@localhost:5432", NoTls)
//!         .await
//!         .expect("couln't connect to database");
//!     tokio::spawn(async move { conn.await.unwrap() });
//! 
//!     pg_worm::register!(Book).await.unwrap();
//! 
//!     let client = get_client().unwrap();
//! 
//!     client.execute(
//!         "INSERT INTO book (title) VALUES ($1)",
//!         &[&"Bible"]
//!     ).await.unwrap();
//! 
//!     let books = client.query("SELECT id, title FROM book ORDER BY id", &[]).await.unwrap();
//! 
//!     let bible = Book::from_row(books.first().unwrap()).unwrap();
//!     assert_eq!(bible.title, "Bible");
//!     assert_eq!(bible.id, 1);
//! }
//! ```

// This allows importing this crate's contents from pg-worm-derive.
extern crate self as pg_worm;

pub use pg_worm_derive::*;
pub use tokio_postgres::{self, config, Client, NoTls, Row};

use once_cell::sync::OnceCell;
use thiserror::Error;
use tokio_postgres::{tls::MakeTlsConnect, Connection, Socket};

#[derive(Error, Debug)]
pub enum PgWormError {
    #[error("couldn't connect to database")]
    ConnectionError,
    #[error("already connected to database")]
    AlreadyConnected,
    #[error("not connected to database")]
    NotConnected,
    #[error("pg error")]
    PostgresError(#[from] tokio_postgres::Error),
}

static CLIENT: OnceCell<Client> = OnceCell::new();

/// Get a reference to the client, if a connection has been made
/// returns `None` otherwise.
#[inline]
#[must_use]
pub fn get_client() -> Option<&'static Client> {
    CLIENT.get()
}

/// Connect `pg_worm` to a postgres database.
///
/// You need to activate the connection by spawning it of into a different thread, only then will the client actually work.
/// ```ignore
/// let conn = connect("my_db_url", NoTls).expect("db connection failed");
/// tokio::spawn(async move {
///     conn.await.unwrap()
/// });
/// ```
pub async fn connect<T>(config: &str, tls: T) -> Result<Connection<Socket, T::Stream>, PgWormError>
where
    T: MakeTlsConnect<Socket>,
{
    let (client, conn) = tokio_postgres::connect(config, tls).await?;
    match CLIENT.set(client) {
        Ok(_) => (),
        Err(_) => return Err(PgWormError::AlreadyConnected),
    };
    Ok(conn)
}

/// Register your model with the database.
/// This creates a table representing your model.
pub async fn register_model<M: Model<M>>() -> Result<(), PgWormError> {
    if let Some(client) = get_client() {
        client.execute(&M::create_sql(), &[]).await?;
        return Ok(());
    }

    Err(PgWormError::NotConnected)
}

#[macro_export]
macro_rules! register {
    ($x:ty) => {
        $crate::register_model::<$x>()
    };
}

/// This is the trait which you should derive for your model structs.
///
/// It will provide the ORM functionality.
pub trait Model<T> {
    /// Parse a `tokio_postgres::Row` to your model.
    fn from_row(row: &Row) -> Result<T, tokio_postgres::Error>;

    #[must_use]
    fn create_sql() -> String;
}

#[cfg(test)]
mod tests {
    use pg_worm::Model;

    #[derive(Model)]
    struct Person {
        #[column(dtype = "BIGSERIAL", primary_key, unique)]
        id: i64,
        #[column(dtype = "TEXT")]
        name: String,
    }

    #[test]
    fn sql_create_table() {
        assert_eq!(
            Person::create_sql(),
            "CREATE TABLE IF NOT EXISTS person (id BIGSERIAL PRIMARY KEY UNIQUE, name TEXT)"
        );
    }
}
