# *P*ost*g*reSQL's *W*orst *ORM*

The goal is for `pg-worm` to become a straight forward and easy-to-use ORM.

Currently this means providing a derivable `Model` trait which provides 
various functionality.

## Usage

```rust
use pg_worm::{register, connect, Model, NoTls};

#[derive(Model)]
struct Book {
    #[column(dtype = "BIGSERIAL", primary_key, unique)]
    id: i64,
    #[column(dtype = "TEXT", unique)]
    title: String,
}

#[tokio::main]
async fn main() -> Result<(), pg_worm::Error> {
    // First create a connection. This can be only done _once_.
    let conn = connect("postgres://me:me@localhost:5432", NoTls).await?;
    // Boilerplate needed for the connection to start listening.
    tokio::spawn(async move { conn.await.unwrap() });

    // First, register the model with the pg_worm client.
    //
    // This creates a completely new table.
    // Beware that should there already be a table
    // with the same name, it is dropped.
    register!(Book).await?;

    // Next, insert a new book.
    // This works by passing values for all
    // fields which aren't autogenerated.
    Book::insert(
        "Foo - Part II".to_string()
    ).await?;

    // Query all entities from the database
    let books = Book::select().await;

    assert_eq!(books.len(), 1);
    assert_eq!(books[0].title, "Foo - Part II");

    // Alternatively, query exactly one book.
    let book: Option<Book> = Book::select_one().await;
    assert!(book.is_some());

    Ok(())
}
```