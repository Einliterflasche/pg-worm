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

    pg_worm::register!(Book).await.unwrap();

    let client = get_client().unwrap();

    client.execute(
        "INSERT INTO book (title) VALUES ($1)",
        &[&"Bible"]
    ).await.unwrap();

    let books = client.query("SELECT id, title FROM book ORDER BY id", &[]).await.unwrap();

    let bible = Book::from_row(books.first().unwrap()).unwrap();
    assert_eq!(bible.title, "Bible");
    assert_eq!(bible.id, 1);
}
```
