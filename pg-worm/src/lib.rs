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
//! use pg_worm::{register, connect, NoTls, Model, Filter};
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
//!     register!(User).await?;
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

use std::{
    marker::PhantomData,
    ops::{BitAnd, BitOr},
};

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

    fn combine_with_sep(mut f1: Filter, f2: Filter, sep: &str) -> Filter {
        let mut left_stmt = f1.stmt + sep;
        let mut right_stmt = f2.stmt;

        while let Some(i) = right_stmt.find('$') {
            // Compute number of digits of the current placeholder number
            let mut digs: usize = 0usize;
            loop {
                let slice = &right_stmt[i + 1 + digs..];
                if let Some(c) = slice.chars().next() {
                    if c.is_numeric() {
                        digs += 1;
                        continue;
                    }
                }
                break;
            }

            // Parse the number
            let num: usize = right_stmt[i + 1..=i + digs].parse().unwrap();

            // Add everything before the number to the left stmt
            // assert!(curr <= i, "!{curr} <= {i}");
            left_stmt.push_str(&right_stmt[..=i]);
            // Add the new number to the left statement
            left_stmt.push_str(&format!("{}", num + f1.args.len()));
            // Repeat for the rest of the placeholders
            let new_start = i + digs + 1;
            right_stmt = right_stmt[new_start..].to_string();
        }

        // Add rest if the string
        left_stmt += &right_stmt;

        f1.args.extend(f2.args);

        Filter::new(left_stmt, f1.args)
    }
}

impl BitAnd for Filter {
    type Output = Filter;

    fn bitand(self, rhs: Self) -> Self::Output {
        Filter::combine_with_sep(self, rhs, " AND ")
    }
}

impl BitOr for Filter {
    type Output = Filter;

    fn bitor(self, rhs: Self) -> Self::Output {
        Filter::combine_with_sep(self, rhs, " OR ")
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
        Filter::new(format!("{} = $1", self.name), vec![Box::new(value.into())])
    }

    pub fn neq(&self, value: impl Into<T>) -> Filter {
        Filter::new(format!("{} != $1", self.name), vec![Box::new(value.into())])
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

        Filter::new(format!("{} IN ({placeholders})", self.name), vals)
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

        Filter::new(format!("{} NOT IN ({placeholders})", self.name), vals)
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
    fn table_creation_sql() {
        assert_eq!(
            Person::_table_creation_sql(),
            "DROP TABLE IF EXISTS personas CASCADE; CREATE TABLE personas (id int8 PRIMARY KEY GENERATED ALWAYS AS IDENTITY, name text)"
        );
    }
}
