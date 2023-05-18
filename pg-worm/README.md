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
use pg_worm::{register, connect, NoTls, Model};

#[derive(Model)]
#[table(table_name = "users")]                  // Postgres doesn't allow tables named `user`
                                                // - no problem! Simply rename the table.
struct User {
    #[column(primary_key, auto)]                // Set a primary key which automatically increments    
    id: i64,
    #[column(unique)]                           // Enable the uniqueness constraint
    name: String,
    #[column(column_name = "pwd_hash")]         // You can rename columns too
    password: String
} 

#[tokio::main]
async fn main() -> Result<(), pg_worm::Error> {
    // First, connect to your server.
    let conn = connect("postgres://me:me@localhost:5432", NoTls).await?;
    // Sadly, there is still boilerplate necessary for
    // actually starting the connection. We are working on 
    // making this more convenient too.
    tokio::spawn(async move { conn.await.expect("unable to connect") });

    // Finally, register your `Model`.
    // This creates a new table, but be aware
    // that any old table with the same name 
    // will be dropped and you _will_ lose your data.
    register!(User).await.unwrap();

    // Now you can start doing what you really
    // want to do - after just 3 lines of setup.

    // First, we will create some new users.
    // Notice, how you can pass `&str` as well as `String` 
    // - convenient, isn't it?
    User::insert("Bob", "very_hashed_password").await?;
    User::insert("Kate".to_string(), "another_hashed_password").await?;

    // Querying data is just as easy:
    // Retrieve all entities there are...
    let all_users: Vec<User> = User::select().await;     
    assert_eq!(all_users.len(), 2);
    
    // Or just (n)one
    let first_user: Option<User> = User::select_one().await;
    assert!(first_user.is_some());
    assert_eq!(first_user.unwrap().name, "Bob");
    
    Ok(())
}
```
