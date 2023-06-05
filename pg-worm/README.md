![GitHub Actions Testing](https://github.com/Einliterflasche/pg-worm/actions/workflows/rust.yml/badge.svg)

# `pg-worm`
### *P*ost*g*reSQL's *W*orst *ORM*
`pg-worm` is an opiniated, straightforward, async ORM for PostgreSQL servers.
Well, at least that's the goal. 

This library is based on [`tokio_postgres`](https://docs.rs/tokio-postgres/0.7.8/tokio_postgres/index.html) 
and is intended to be used with [`tokio`](https://tokio.rs/).

## Usage
Fortunately, using this library is very easy.

Just derive the `Model` trait for your type, connect to your database 
and you are ready to go!

Here's a quick example: 

```rust
use pg_worm::{register, connect, NoTls, Model, Filter};

#[derive(Model)]
// Postgres doesn't allow tables named `user`
#[table(table_name = "users")]
struct User {
    // A primary key which automatically increments
    #[column(primary_key, auto)]
    id: i64,
    // A column which requires unique values
    #[column(unique)]
    name: String,
    // You can rename columns too
    #[column(column_name = "pwd_hash")]
    password: String
} 

#[tokio::main]
async fn main() -> Result<(), pg_worm::Error> {
    // Simply connect to your server...
    connect!("postgres://me:me@localhost:5432", NoTls).await?;

    // ...and then register your `Model`.
    // This creates a new table. Be aware
    // that any old table with the same name 
    // will be dropped and you _will_ lose your data.
    register!(User).await?;

    // Now start doing what you actually care about.

    // First, we will create some new users.
    User::insert("Bob", "very_hashed_password").await?;
    User::insert("Kate", "another_hashed_password").await?;

    // Querying data is just as easy:

    // Retrieve all users there are...
    let all_users: Vec<User> = User::select(Filter::all()).await;     
    assert_eq!(all_users.len(), 2);
    
    // Or look for Bob...
    let bob: Option<User> = User::select_one(User::name.eq("Bob")).await;
    assert!(bob.is_some()); // Found him
    assert_eq!(bob.unwrap().name, "Bob");
    
    // Or delete Bob
    User::delete(User::name.eq("Bob")).await;

    // Graceful shutdown
    Ok(())
}
```

## Filters
Filters can be used to easily include `WHERE` clauses in your queries. 

They can be constructed by calling functions of the respective column. 
`pg_worm` automatically constructs a `Column` constant for each field 
of your `Model`. 

A practical example would look like this:

```rust
MyModel::select(MyModel::my_field.eq(5))
```

Currently the following filter functions are supported:

 * `Filter::all()` - doesn't check anything
 * `eq(T)` - checks whether the column value is equal to something
 * `neq(T)` - checks whether the column value is not equal to something
 * `one_of(Vec<T>)` - checks whether the vector contains the column value.
 * `none_of(Vec<T>)` - checks whether the vector does not contain the column value.
 
You can also do filter logic using `&` and `|`: `MyModel::my_field.eq(5) & MyModel::other_field.neq("Foo")`.
This works as you expect logical OR and AND to work.
Please notice that, at this point, custom priorization via parantheses 
is **not possible**.

## Opiniatedness
As mentioned before, `pg_worm` is opiniated in a number of ways. 
These include:

 * `panic`s. For the sake of convenience `pg_worm` only returns a  `Result` when 
   inserting data, since in that case Postgres might reject the data because of
   some constraint. 

   This means that should something go wrong, like:
    - the connection to the database collapsed,
    - `pg_worm` is unable to parse Postgres' response,
    - ...
   
   the program will panic.
 * ease of use. The goal of `pg_worm` is **not** to become an enterprise solution.
   If adding an option means infringing the ease of use then it will likely
   not be added.

## License
This project is dual-licensed under the MIT and Apache 2.0 licenses.
