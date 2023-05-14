# *P*ost*g*reSQL's *W*orst *ORM*

The goal is for `pg-worm` to become an easy-to-use ORM (unlike, say, diesel-rs).

Currently this means providing a derivable `Model` 
trait for easily parsing structs from `tokio_postgres::Model`.

## Usage

```rust
use pg_worm::{Model, NoTls, connect};

#[derive(Model)]
struct Person {
    #[column(dtype = "BIGSERIAL")]
    id: i64,
    #[column(dtype = "TEXT")]
    name: String
}

#[tokio::main]
async fn main() {
    let conn = connect("postgres://me:me@localhost:5432", NoTls).await.unwrap();
    tokio::spawn(async move { conn.await.unwrap() } );

    let client = pg_worm::get_client().expect("unable to connect to database");

    client
        .execute(
            "CREATE TABLE IF NOT EXISTS person (
                id BIGSERIAL PRIMARY KEY UNIQUE,
                name TEXT
            )",
            &[]
        ).await.unwrap();

    client
        .execute(
            "INSERT INTO person (name) VALUES ($1)",
            &[&"Jesus"]
        ).await.unwrap();

    let rows = client.query("SELECT id, name FROM person", &[]).await.unwrap();

    let person = Person::from_row(rows.first().unwrap()).unwrap();
    assert_eq!(person.name, "Jesus");
    assert_eq!(person.id, 1)
}
```
