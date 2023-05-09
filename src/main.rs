use pg_worm::Entity;
use tokio_postgres::{NoTls, connect};

#[derive(Debug, Entity)]
struct Person {
    id: i64,
    name: String
}

#[tokio::main]
async fn main() {
    let (client, conn) = connect("postgresql://me:me@localhost:5432", NoTls).await.unwrap();
    tokio::spawn(async move { conn.await.unwrap()} );

    client.execute(
        "CREATE TABLE IF NOT EXISTS person (
            id BIGSERIAL PRIMARY KEY UNIQUE,
            name TEXT
        )",
        &[]
    ).await.unwrap();

    client.execute("INSERT INTO person (name) VALUES ($1)", &[&"Jesus"]).await.unwrap();

    let rows = client.query("SELECT id, name FROM person", &[]).await.unwrap();

    let person = Person::from_sql(&rows[0]).unwrap();
    println!("Person: {:#?}", person)
}
