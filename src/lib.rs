//! The goal is to create a simple, easy-to-use ORM for PostgreSQL databases.
//!
//! Currently only offers a derivable `Model` trait for parsing structs from `tokio_postgres::Row`.
//!
//! # Usage
//!
//! ```
//! use pg_worm::{Model, NoTls, connect};
//!
//! #[derive(Model)]
//! struct Person {
//!     #[column(dtype = "BIGSERIAL")]
//!     id: i64,
//!     #[column(dtype = "TEXT")]
//!     name: String
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let conn = connect("postgres://me:me@localhost:5432", NoTls).await.unwrap();
//!     tokio::spawn(async move { conn.await.unwrap() } );
//!     
//!     let client = pg_worm::get_client().expect("unable to connect to database");
//!
//!     client
//!         .execute(
//!             "CREATE TABLE IF NOT EXISTS person (
//!                 id BIGSERIAL PRIMARY KEY UNIQUE,
//!                 name TEXT
//!             )",
//!             &[]
//!         ).await.unwrap();
//!
//!     client
//!         .execute(
//!             "INSERT INTO person (name) VALUES ($1)",
//!             &[&"Jesus"]
//!         ).await.unwrap();
//!
//!     let rows = client.query("SELECT id, name FROM person", &[]).await.unwrap();
//!
//!     let person = Person::from_row(rows.first().unwrap()).unwrap();
//!     assert_eq!(person.name, "Jesus");
//!     assert_eq!(person.id, 1)
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
