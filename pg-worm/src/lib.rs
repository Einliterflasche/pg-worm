//! # `pg-worm`
//! ### *P*ost*g*reSQL's *W*orst *ORM*
//! `pg-worm` is an opiniated, straightforward, async ORM for PostgreSQL servers.
//! Well, at least that's the goal.
//!
//! This library is based on [`tokio_postgres`](https://docs.rs/tokio-postgres/0.7.8/tokio_postgres/index.html)
//! and is intended to be used with [`tokio`](https://tokio.rs/).
//!
//! ## Usage
//! Fortunately, using this library is very easy.
//!
//! Just derive the [`Model`] trait for your type, connect to your database
//! and you are ready to go!
//!
//! Here's a quick example:
//!
//! ```
//! use pg_worm::prelude::*;
//! use tokio::try_join;
//!
//! // First easily define your models.
//! #[derive(Model)]
//! struct Book {
//!     // `id` will be the primary key column and
//!     // automatically generated/incremented
//!     #[column(primary_key, auto)]
//!     id: i64,
//!     #[column(unique)]
//!     title: String,
//!     author_id: i64,
//! }
//!
//! #[derive(Model)]
//! struct Author {
//!     #[column(primary_key, auto)]
//!     id: i64,
//!     name: String,
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), pg_worm::Error> {
//!     // First connect to your server. This can be only done once.
//!     connect!("postgres://me:me@localhost:5432", NoTls).await?;
//!
//!     // Then, create tables for your models.
//!     // Use `register!` if you want to fail if a
//!     // table with the same name already exists.
//!     //
//!     // `force_register` drops the old table,
//!     // which is useful for development.
//!     //
//!     // If your tables already exist, skip this part.
//!     force_register!(Author, Book)?;
//!
//!     // Next, insert some data.
//!     // This works by passing values for all
//!     // fields which aren't autogenerated.
//!     try_join!(
//!         Author::insert("Stephen King"),
//!         Author::insert("Martin Luther King"),
//!         Author::insert("Karl Marx"),
//!         Book::insert("Foo - Part I", 1),
//!         Book::insert("Foo - Part II", 2),
//!         Book::insert("Foo - Part III", 3)
//!     )?;
//!
//!     // Do a simple query for all books
//!     let books = Book::select().await?;
//!     assert_eq!(books.len(), 3);
//!
//!     // Graceful shutdown
//!     Ok(())
//! }
//! ```
//!
//! ## Filters
//! [`Filter`]s can be used to easily include `WHERE` clauses in your queries.
//!
//! They can be constructed by calling functions of the respective column.
//! `pg_worm` automatically constructs a [`TypedColumn`] constant for each field
//! of your `Model`.
//!
//! A practical example would look like this:
//!
//! ```ignore
//! MyModel::select(MyModel::my_field.eq(5))
//! ```
//!
//! Currently the following filter functions are supported:
//!
//!  * `Filter::all()` - doesn't check anything
//!  * `eq(T)` - checks whether the column value is equal to something
//!  * `one_of(Vec<T>)` - checks whether the vector contains the column value.
//!  
//! You can also do filter logic using `!`, `&` and `|`: `MyModel::my_field.eq(5) & !MyModel::other_field.eq("Foo")`.
//! This works as you expect logical OR and AND to work.
//! Please notice that, at this point, custom priorization via parantheses
//! is **not possible**.
//!
//!
//! ## Query Builder
//! Simply attaching a [`Filter`] to your query often does not suffice.
//! For this reason, `pg-worm` provides a `QueryBuilder` interface for
//! constructing more complex queries.
//!
//! Start building your query by calling `Query::select()` and passing
//! the columns you want to select.
//! Normally you want to query all columns of a `Model` which you can do by passing
//! `YourModel::columns()`.
//!
//! You can modify your query using the following methods:
//!
//!  * `.filter()` - add a `WHERE` clause
//!  * `.join()` - add a `JOIN` for querying accross tables/models
//!  * `.limit()` - add a `LIMIT` to how many rows are returned
//!
//! After you have configured your query, build it using the `.build()` method.
//! Then, execute it by calling `.exec::<M>()`, where `M` is the `Model` which
//! should be parsed from the query result. It may be inferred.
//!
//! ## Opiniatedness
//! As mentioned before, `pg_worm` is opiniated in a number of ways.
//! These include:
//!
//!  * `panic`s. For the sake of convenience `pg_worm` only returns a  `Result` when
//!    inserting data, since in that case Postgres might reject the data because of
//!    some constraint.
//!
//!    This means that should something go wrong, like:
//!     - the connection to the database collapsed,
//!     - `pg_worm` is unable to parse Postgres' response,
//!     - ...
//!
//!    the program will panic.
//!  * ease of use. The goal of `pg_worm` is **not** to become an enterprise solution.
//!    If adding an option means infringing the ease of use then it will likely
//!    not be added.

#![deny(missing_docs)]

// This allows importing this crate's contents from pg-worm-derive.
extern crate self as pg_worm;

pub mod query;

use std::ops::Deref;

pub use query::{Column, TypedColumn};

pub use async_trait::async_trait;
pub use pg::{NoTls, Row};
pub use pg_worm_derive::Model;
use prelude::Select;
/// This crate's reexport of the `tokio_postgres` crate.
pub use tokio_postgres as pg;

use once_cell::sync::OnceCell;
use pg::{tls::MakeTlsConnect, Client, Connection, Socket};
use thiserror::Error;

/// This module contains all necessary imports to get you started
/// easily. 
pub mod prelude {
    pub use crate::{
        Model,
        connect, 
        NoTls,
        force_register, 
        register,
    };

    pub use crate::query::{Column, TypedColumn, Select};
    pub use std::ops::Deref;
}

/// An enum representing the errors which are emitted by this crate.
#[derive(Error, Debug)]
pub enum Error {
    /// Something went wrong while connection to the database.
    #[error("couldn't connect to database")]
    ConnectionError,
    /// There already is a connection to the database.
    #[error("already connected to database")]
    AlreadyConnected,
    /// No connection has yet been established.
    #[error("not connected to database")]
    NotConnected,
    /// Errors emitted by the Postgres server.
    /// 
    /// Most likely an invalid query.
    #[error("error communicating with database")]
    PostgresError(#[from] tokio_postgres::Error),
}

/// This is the trait which you should derive for your model structs.
///
/// It provides the ORM functionality.
///
#[async_trait]
pub trait Model<T>: TryFrom<Row, Error = Error> {
    /// This is a library function needed to derive the `Model`trait.
    ///
    /// *_DO NOT USE_*
    #[doc(hidden)]
    #[must_use]
    fn _table_creation_sql() -> &'static str;

    /// Returns a slice of all columns this model's table has.
    fn columns() -> &'static [&'static dyn Deref<Target = Column>];

    /// Returns the name of this model's table's name.
    fn table_name() -> &'static str;

    /// Start building a `SELECT` query which will be parsed to this model.
    fn select<'a>() -> Select<'a, Vec<T>>;

    /// Start building a `SELECT` query which returns either
    /// one entity or `None`.
    fn select_one<'a>() -> Select<'a, Option<T>>;
}

static CLIENT: OnceCell<Client> = OnceCell::new();

/// Get a reference to the client, if a connection has been made.
/// Returns `Err(Error::NotConnected)` otherwise.
///
/// **This is a private library function needed to derive
/// the `Model` trait. Do not use!**
#[doc(hidden)]
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
/// Calls the [`connect()`] function.
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
/// Use the [`register!`] macro for a more convenient api.
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
pub async fn register_model<M: Model<M>>() -> Result<(), Error>
where
    Error: From<<M as TryFrom<Row>>::Error>,
{
    let client = _get_client()?;
    client.batch_execute(M::_table_creation_sql()).await?;

    Ok(())
}

/// Same as [`register_model`] but if a table with the same name
/// already exists, it is dropped instead of returning an error.
pub async fn force_register_model<M: Model<M>>() -> Result<(), Error>
where
    Error: From<<M as TryFrom<Row>>::Error>,
{
    let client = _get_client()?;
    let query = format!(
        "DROP TABLE IF EXISTS {} CASCADE; ",
        M::columns()[0].table_name()
    ) + M::_table_creation_sql();

    client.batch_execute(&query).await?;

    Ok(())
}

/// Registers a [`Model`] with the database by creating a
/// corresponding table.
///
/// This is just a more convenient version api
/// for the [`register_model`] function.
///
/// This macro, too, requires the `tokio` crate.
///
/// Returns an error if:
///  - a table with the same name already exists,
///  - the client is not connected,
///  - the creation of the table fails
///
/// # Usage
///
/// ```ignore
/// use pg_worm::{Model, register};
///
/// #[derive(Model)]
/// struct Foo {
///     #[column(primary_key)]
///     id: i64
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<(), pg_worm::Error> {
///     // ---- snip connection setup ----
///     register!(Foo)?;
/// }
/// ```
#[macro_export]
macro_rules! register {
    ($($x:ty),+) => {
        tokio::try_join!(
            $($crate::register_model::<$x>()),*
        )
    };
}

/// Like [`register!`] but if a table with the same name already
/// exists, it is dropped instead of returning an error.
#[macro_export]
macro_rules! force_register {
    ($($x:ty),+) => {
        tokio::try_join!(
            $($crate::force_register_model::<$x>()),*
        )
    };
}
