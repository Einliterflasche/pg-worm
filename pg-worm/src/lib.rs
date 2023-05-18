//! The goal is to create a simple, easy-to-use ORM for PostgreSQL databases.

// This allows importing this crate's contents from pg-worm-derive.
extern crate self as pg_worm;

pub use async_trait::async_trait;
pub use pg::{NoTls, Row};
pub use pg_worm_derive::Model;
/// This crate's reexport of the `tokio_postgres` crate.
pub use tokio_postgres as pg;

use once_cell::sync::OnceCell;
use pg::{tls::MakeTlsConnect, Client, Connection, Socket};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("couldn't connect to database")]
    ConnectionError,
    #[error("already connected to database")]
    AlreadyConnected,
    #[error("not connected to database")]
    NotConnected,
    #[error("error communicating with database")]
    PostgresError(#[from] tokio_postgres::Error),
}

/// This is the trait which you should derive for your model structs.
///
/// It provides the ORM functionality.
///
#[async_trait]
pub trait Model<T>: for<'a> TryFrom<&'a Row> {
    /// This is a library function needed to derive the `Model`trait.
    ///
    /// *_DO NOT USE_*
    #[must_use]
    fn _create_table_sql() -> &'static str;

    /// Retrieve all entities from the table.
    ///
    /// # Panics
    /// For the sake of convenience this function does not return
    /// a `Result` but panics instead
    ///  - if there is no database connection
    #[must_use]
    async fn select() -> Vec<T>;

    /// Retrieve the first entity from the database.
    /// Returns `None` if there are no entities present.
    ///
    /// # Panics
    /// For the sake of convenience this function does not return
    /// a `Result` but panics instead
    ///  - if there is no database connection
    #[must_use]
    async fn select_one() -> Option<T>;
}

static CLIENT: OnceCell<Client> = OnceCell::new();

/// Get a reference to the client, if a connection has been made.
/// Returns `Err(Error::NotConnected)` otherwise.
#[inline]
pub fn _get_client() -> Result<&'static Client, Error> {
    if let Some(client) = CLIENT.get() {
        Ok(client)
    } else {
        Err(Error::NotConnected)
    }
}

/// Connect the `pg_worm` client to a postgres database.
///
/// You need to *_activate the connection by spawning it off into a new thread_*, only then will the client actually work.
///
/// You can connect to a database only once. If you try to connect again,
/// the function will return an error.
///
/// # Example
/// ```ignore
/// let conn = connect("my_db_url", NoTls).expect("db connection failed");
/// tokio::spawn(async move {
///     conn.await.expect("connection error")
/// });
/// ```
pub async fn connect<T>(config: &str, tls: T) -> Result<Connection<Socket, T::Stream>, Error>
where
    T: MakeTlsConnect<Socket>,
{
    let (client, conn) = tokio_postgres::connect(config, tls).await?;
    match CLIENT.set(client) {
        Ok(_) => (),
        Err(_) => return Err(Error::AlreadyConnected),
    };
    Ok(conn)
}

/// Register your model with the database.
/// This creates a table representing your model.
///
/// Use the `register!` macro for a more convenient api.
///
/// # Usage
/// ```ignore
/// #[derive(Model)]
/// struct Foo {
///     #[column(dtype = "BIGSERIAL")]
///     id: i64
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<(), pg_worm::Error> {
///     // ---- snip connection setup ----
///     pg_worm::register_model::<M>().await?;
/// }
/// ```
pub async fn register_model<M: Model<M>>() -> Result<(), Error> {
    let client = _get_client()?;
    client.batch_execute(M::_create_table_sql()).await?;

    Ok(())
}

/// Registers a `Model` with the database by creating a
/// corresponding table.
///
/// This is just a more convenient version api
/// for the `register_model<M>` function.
///
/// If a table  with the same name already
/// exists, it is dropped.
///
/// Returns an error if:
///  - the client is not connected
///  - the creation of the table fails
///
/// # Usage
///
/// ```ignore
/// use pg_worm::{Model, register};
///
/// #[derive(Model)]
/// struct Foo {
///     #[column(dtype = "BIGSERIAL")]
///     id: i64
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<(), pg_worm::Error> {
///     // ---- snip connection setup ----
///     register!(Model).await?;
/// }
/// ```
#[macro_export]
macro_rules! register {
    ($x:ty) => {
        $crate::register_model::<$x>()
    };
}

#[cfg(test)]
mod tests {
    use pg_worm::Model;

    #[derive(Model)]
    #[table(table_name = "personas")]
    struct Person {
        #[column(dtype = "BIGSERIAL", primary_key, unique)]
        id: i64,
        #[column(dtype = "TEXT")]
        name: String,
    }

    #[test]
    fn sql_create_table() {
        assert_eq!(
            Person::_create_table_sql(),
            "DROP TABLE IF EXISTS personas CASCADE; CREATE TABLE personas (id BIGSERIAL PRIMARY KEY UNIQUE, name TEXT)"
        );
    }
}
