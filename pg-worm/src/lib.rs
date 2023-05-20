//! `pg-worm` is an opiniated, straightforward, async ORM for PostgreSQL servers.
//! Well, at least that's the goal.
//!
//! This library is based on [`tokio_postgres`](https://docs.rs/tokio-postgres/0.7.8/tokio_postgres/index.html)
//! and is intended to be used with [`tokio`](https://tokio.rs/).
//!
//! # Usage
//!
//! Fortunately, using this library is very easy.
//!
//! Just derive the `Model` trait for your type, connect to your database
//! and you are ready to go!
//!
//! Here's a quick example:
//!
//! ```
//! use pg_worm::{register, connect, NoTls, Model, Filter};
//!
//! #[derive(Model)]
//! #[table(table_name = "users")]                  // Postgres doesn't allow tables named `user`
//!                                                 // - no problem! Simply rename the table.
//! struct User {
//!     #[column(primary_key, auto)]                // Set a primary key which automatically increments    
//!     id: i64,
//!     #[column(unique)]                           // Enable the uniqueness constraint
//!     name: String,
//!     #[column(column_name = "pwd_hash")]         // You can rename columns too
//!     password: String
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), pg_worm::Error> {
//!     // Simply connect to your server.
//!     connect!("postgres://me:me@localhost:5432", NoTls).await?;
//!
//!     // Then register your `Model`.
//!     // This creates a new table, but be aware
//!     // that any old table with the same name
//!     // will be dropped and you _will_ lose your data.
//!     register!(User).await.unwrap();
//!
//!     // Now you can start doing what you really
//!     // want to do - after just 3 lines of setup.
//!
//!     // First, we will create some new users.
//!     // Notice, how you can pass `&str` as well as `String`
//!     // - convenient, isn't it?
//!     User::insert("Bob", "very_hashed_password").await?;
//!     User::insert("Kate".to_string(), "another_hashed_password").await?;
//!
//!     // Querying data is just as easy:
//!
//!     // Retrieve all entities there are...
//!     let all_users: Vec<User> = User::select(Filter::all()).await;     
//!     assert_eq!(all_users.len(), 2);
//!     
//!     // Or just one...
//!     let bob: Option<User> = User::select_one(User::name.eq("Bob")).await;
//!     assert!(bob.is_some());
//!     assert_eq!(bob.unwrap().name, "Bob");
//!     
//!     // Graceful shutdown
//!     Ok(())
//! }
//! ```
//!
//!
//! ## Filters
//! Filters are way to easily using `WHERE` clauses in your queries.
//!
//! Unless otherwise specified they are methods on the column constants and can be called like so:
//!
//! ```ignore
//! MyModel::select(MyModel::my_field.eq(5));
//! ```
//!
//! Currently the following filter functions are supported:
//!
//!  * `Filter::all()` - doesn't check anything
//!  * `eq(val)` - checks whether the column value is equal to something
//!  * `neq(val)` - checks whether the column value is not equal to something
//!  * `one_of(Vec<val>)` - checks whether the column value is one of the ones specified
//!  * `none_of(Vec<val>)` - checks whether the column value is not one of the ones specified

// This allows importing this crate's contents from pg-worm-derive.
extern crate self as pg_worm;

use std::marker::PhantomData;

pub use async_trait::async_trait;
pub use pg::{NoTls, Row};
pub use pg_worm_derive::Model;
/// This crate's reexport of the `tokio_postgres` crate.
pub use tokio_postgres as pg;

use once_cell::sync::OnceCell;
use pg::{tls::MakeTlsConnect, types::ToSql, Client, Connection, Socket};
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
    fn _table_creation_sql() -> &'static str;

    /// Retrieve all entities from the table.
    ///
    /// # Panics
    /// For the sake of convenience this function does not return
    /// a `Result` but panics instead
    ///  - if there is no database connection
    #[must_use]
    async fn select(filter: Filter) -> Vec<T>;

    /// Retrieve the first entity from the database.
    /// Returns `None` if there are no entities present.
    ///
    /// # Panics
    /// For the sake of convenience this function does not return
    /// a `Result` but panics instead
    ///  - if there is no database connection
    #[must_use]
    async fn select_one(filter: Filter) -> Option<T>;

    /// Delete any entity wich matches the filter.
    ///
    /// Returns the number of rows affected.
    ///
    /// # Panic
    /// For the sake of convenience this function does not return
    /// a `Result` but panics instead
    ///  - if there is no database connection
    async fn delete(filter: Filter) -> u64;
}

static CLIENT: OnceCell<Client> = OnceCell::new();

/// Get a reference to the client, if a connection has been made.
/// Returns `Err(Error::NotConnected)` otherwise.
///
/// **This is a private library function needed to derive
/// the `Model` trait. Do not use!**
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

/// Convenience macro for connecting the `pg-worm` client
/// to a database server. Essentially writes the boilerplate
/// code needed. See the [`tokio_postgres`](https://docs.rs/tokio-postgres/latest/tokio_postgres/config/struct.Config.html)
/// documentation for more information on the config format.
///
/// Needs `tokio` to work.
///
/// # Panics
/// Panics when the connection is closed due to a fatal error.
#[macro_export]
macro_rules! connect {
    ($config:literal, $tls:expr) => {
        async {
            match $crate::connect($config, $tls).await {
                Ok(conn) => {
                    tokio::spawn(async move { conn.await.expect("fatal connection error") });
                    return Ok(());
                }
                Err(err) => return Err(err),
            }
        }
    };
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
///     #[column(primary_key)]
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
    client.batch_execute(M::_table_creation_sql()).await?;

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
///     #[column(primary_ley)]
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

/// Struct for filtering your queries.
///
/// _These are automatically generated by operations
/// like `MyModel::my_field.eq(5)`. **You are not supposed to
/// construct them manually.**_
///
/// Stores the statement
/// and arguments. The statement should include placeholders
/// in the form of `$1`, `$2` and so on.
pub struct Filter {
    stmt: String,
    args: Vec<Box<dyn ToSql + Sync + Send>>,
}

impl Filter {
    fn new(stmt: impl Into<String>, args: Vec<Box<dyn ToSql + Sync + Send>>) -> Filter {
        Filter {
            stmt: stmt.into(),
            args,
        }
    }

    /// Creates a new filter which doesn't filter anything.
    pub fn all() -> Filter {
        Filter::new("", Vec::new())
    }

    /// Access the filter's raw sql statement.
    ///
    #[inline]
    pub fn _stmt(&self) -> &str {
        self.stmt.as_str()
    }

    #[inline]
    pub fn _args(&self) -> &Vec<Box<dyn ToSql + Sync + Send>> {
        &self.args
    }
}

pub struct Column<T: ToSql + Sync> {
    name: &'static str,
    rs_type: PhantomData<T>,
}

impl<T: ToSql + Sync + Send + 'static> Column<T> {
    pub const fn new(name: &'static str) -> Column<T> {
        Column {
            name,
            rs_type: PhantomData::<T>,
        }
    }

    pub fn eq(&self, value: impl Into<T>) -> Filter {
        Filter::new(
            format!("WHERE {} = $1", self.name),
            vec![Box::new(value.into())],
        )
    }

    pub fn neq(&self, value: impl Into<T>) -> Filter {
        Filter::new(
            format!("WHERE {} != $1", self.name),
            vec![Box::new(value.into())],
        )
    }

    pub fn one_of(&self, values: Vec<impl Into<T>>) -> Filter {
        // Early return if no values are supplied
        if values.is_empty() {
            return Filter::all();
        }

        // Generate the placeholders for the query
        // like $1, $2, ...
        let placeholders = (1..=values.len())
            .map(|i| format!("${i}"))
            .collect::<Vec<_>>()
            .join(", ");

        // Convert values to needed type
        let vals = values
            .into_iter()
            .map(|i| Box::new(i.into()) as Box<(dyn ToSql + Send + Sync + 'static)>)
            .collect::<Vec<_>>();

        Filter::new(format!("WHERE {} IN ({placeholders})", self.name), vals)
    }

    pub fn none_of(&self, values: Vec<impl Into<T>>) -> Filter {
        // Early return if no values are supplied
        if values.is_empty() {
            return Filter::all();
        }

        // Generate the placeholders for the query
        // like $1, $2, ...
        let placeholders = (1..=values.len())
            .map(|i| format!("${i}"))
            .collect::<Vec<_>>()
            .join(", ");

        // Convert values to needed type
        let vals = values
            .into_iter()
            .map(|i| Box::new(i.into()) as Box<(dyn ToSql + Send + Sync + 'static)>)
            .collect::<Vec<_>>();

        Filter::new(format!("WHERE {} NOT IN ({placeholders})", self.name), vals)
    }
}

#[cfg(test)]
mod tests {
    #![allow(dead_code)]

    use pg_worm::Model;

    #[derive(Model)]
    #[table(table_name = "personas")]
    struct Person {
        #[column(primary_key, auto)]
        id: i64,
        name: String,
    }

    #[test]
    fn sql_create_table() {
        assert_eq!(
            Person::_table_creation_sql(),
            "DROP TABLE IF EXISTS personas CASCADE; CREATE TABLE personas (id int8 PRIMARY KEY GENERATED ALWAYS AS IDENTITY, name text)"
        );
    }
}
