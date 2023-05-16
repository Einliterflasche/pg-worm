# *P*ost*g*reSQL's *W*orst *ORM*

The goal is for `pg-worm` to become an easy-to-use ORM (unlike, say, diesel-rs).

Currently this means providing a derivable `Model` 
trait for easily parsing structs from `tokio_postgres::Model`.

## Usage

```rust
use pg_worm::tokio_postgres::NoTls;
use pg_worm::{connect, Model, get_client};

#[derive(Model)]
struct Book {
    #[column(dtype = "BIGSERIAL", primary_key, unique)]
    id: i64,
    #[column(dtype = "TEXT", unique)]
    title: String,
}

#[tokio::main]
async fn main() {
    let conn = connect("postgres://me:me@localhost:5432", NoTls)
        .await
        .expect("couln't connect to database");
    tokio::spawn(async move { conn.await.unwrap() });

    // Drops and recreates table
    pg_worm::register!(Book).await.unwrap();

    get_client().unwrap().execute(
        "INSERT INTO book (title) VALUES ($1)",
        &[&"Foo"]
    ).await.unwrap();

    let books = Book::select().await.unwrap();

    assert_eq!(books.len(), 1);
    assert_eq!(books[0].title, "Foo");
}
```
