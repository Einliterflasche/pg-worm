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
//! Just derive the `Model` trait for your type, connect to your database
//! and you are ready to go!
//!
//! Here's a quick example:
//!
//! ```
//! use pg_worm::{force_register, connect, NoTls, Model, Filter};
//!
//! #[derive(Model)]
//! // Postgres doesn't allow tables named `user`
//! #[table(table_name = "users")]
//! struct User {
//!     // A primary key which automatically increments
//!     #[column(primary_key, auto)]
//!     id: i64,
//!     // A column which requires unique values
//!     #[column(unique)]
//!     name: String,
//!     // You can rename columns too
//!     #[column(column_name = "pwd_hash")]
//!     password: String
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), pg_worm::Error> {
//!     // Simply connect to your server...
//!     connect!("postgres://me:me@localhost:5432", NoTls).await?;
//!
//!     // ...and then register your `Model`.
//!     // This creates a new table. Be aware
//!     // that any old table with the same name
//!     // will be dropped and you _will_ lose your data.
//!     force_register!(User).await?;
//!
//!     // Now start doing what you actually care about.
//!
//!     // First, we will create some new users.
//!     User::insert("Bob", "very_hashed_password").await?;
//!     User::insert("Kate", "another_hashed_password").await?;
//!
//!     // Querying data is just as easy:
//!
//!     // Retrieve all users there are...
//!     let all_users: Vec<User> = User::select(Filter::all()).await;     
//!     assert_eq!(all_users.len(), 2);
//!     
//!     // Or look for Bob...
//!     let bob: Option<User> = User::select_one(User::name.eq("Bob")).await;
//!     assert!(bob.is_some());
//!     assert_eq!(bob.unwrap().name, "Bob");
//!     
//!     // Or delete Bob, since he does not actually exists
//!     User::delete(User::name.eq("Bob")).await;
//!
//!     // Graceful shutdown
//!     Ok(())
//! }
//! ```
//!
//! ## Filters
//! Filters can be used to easily include `WHERE` clauses in your queries.
//!
//! They can be constructed by calling functions of the respective column.
//! `pg_worm` automatically constructs a `Column` constant for each field
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
//!  * `neq(T)` - checks whether the column value is not equal to something
//!  * `one_of(Vec<T>)` - checks whether the vector contains the column value.
//!  * `none_of(Vec<T>)` - checks whether the vector does not contain the column value.
//!  
//! You can also do filter logic using `&` and `|`: `MyModel::my_field.eq(5) & MyModel::other_field.neq("Foo")`.
//! This works as you expect logical OR and AND to work.
//! Please notice that, at this point, custom priorization via parantheses
//! is **not possible**.
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

// This allows importing this crate's contents from pg-worm-derive.
extern crate self as pg_worm;

pub mod query;

pub use async_trait::async_trait;
pub use pg::{NoTls, Row};
pub use pg_worm_derive::Model;
/// This crate's reexport of the `tokio_postgres` crate.
pub use tokio_postgres as pg;

pub use query::*;

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
    fn _table_creation_sql() -> &'static str;

    fn columns() -> &'static [&'static DynCol];

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

/// Same as `register_model` but if a table with the same name
/// already exists, it is dropped instead of returning an error.
pub async fn force_register_model<M: Model<M>>() -> Result<(), Error> {
    let client = _get_client()?;
    let query = format!(
        "DROP TABLE IF EXISTS {} CASCADE; ",
        M::columns()[0].table_name()
    ) + M::_table_creation_sql();

    client.batch_execute(&query).await?;

    Ok(())
}

/// Registers a `Model` with the database by creating a
/// corresponding table.
///
/// This is just a more convenient version api
/// for the `register_model<M>` function.
///
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
///     #[column(primary_ley)]
///     id: i64
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<(), pg_worm::Error> {
///     // ---- snip connection setup ----
///     register!(Foo).await?;
/// }
/// ```
#[macro_export]
macro_rules! register {
    ($x:ty) => {
        $crate::register_model::<$x>()
    };
}

/// Like `register!` but if a table with the same name already
/// exists, it is dropped instead of returning an error.
#[macro_export]
macro_rules! force_register {
    ($x:ty) => {
        $crate::force_register_model::<$x>()
    };
}

#[cfg(test)]
mod tests {
    #![allow(dead_code)]

    use pg_worm::{Join, JoinType, Model, Query, QueryBuilder};

    use crate::ToQuery;

    #[derive(Model)]
    #[table(table_name = "persons")]
    struct Person {
        #[column(primary_key, auto)]
        id: i64,
        name: String,
    }

    #[derive(Model)]
    struct Book {
        #[column(primary_key, auto)]
        id: i64,
        title: String,
        author_id: i64,
    }

    #[test]
    fn join_sql() {
        assert_eq!(
            Join::new(&Book::author_id, &Person::id, JoinType::Inner).to_sql(),
            "INNER JOIN persons ON book.author_id = persons.id"
        )
    }

    #[test]
    fn select_sql() {
        let q = Query::select([&Book::title])
            .filter(Person::name.like("%a%"))
            .join(&Book::author_id, &Person::id, JoinType::Inner)
            .limit(4)
            .build();

        assert_eq!(
            q.stmt(),
            "SELECT book.title FROM book INNER JOIN persons ON book.author_id = persons.id WHERE persons.name LIKE $1 LIMIT 4"
        )
    }

    #[test]
    fn table_creation_sql() {
        assert_eq!(
            Person::_table_creation_sql(),
            "CREATE TABLE persons (id int8 PRIMARY KEY GENERATED ALWAYS AS IDENTITY, name text)"
        );
    }
}
