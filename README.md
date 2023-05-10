# *P*ost*g*reSQL's *W*orst *ORM*

The goal is for `pg-worm` to be an easy-to-use ORM (unlike, say, diesel-rs).

Currently this means providing a derivable `Model` 
trait for easy parsing structs from `tokio_postgres::Model`.

## Usage

```rust
use pg_worm::Model;
use tokio_postgres::{NoTls, connect};

#[derive(Debug, Model)]
struct Person {
    id: i64,
    name: String
}

#[tokio::main]
async fn main() {
    let (client, conn) = connect("postgres://postgres:postgres@localhost:5432", NoTls).await.unwrap();
    tokio::spawn(async move { conn.await.unwrap() } );

    client.execute(
        "CREATE TABLE IF NOT EXISTS person (
            id BIGSERIAL PRIMARY KEY UNIQUE,
            name TEXT
        )",
        &[]
    ).await.unwrap();

    client.execute("INSERT INTO person (name) VALUES ($1)", &[&"Jesus"]).await.unwrap();

    let rows = client.query("SELECT id, name FROM person", &[]).await.unwrap();

    let person = Person::from_row(&rows[0]).unwrap();
    assert_eq!(person.name, "Jesus");
    assert_eq!(person.id, 1);
}
```
