use pg_worm::{connect, Model};
use pg_worm::tokio_postgres::NoTls;

#[derive(Model)]
struct Book {
    #[column(dtype = "BIGSERIAL")]
    id: i64,
    #[column(dtype = "TEXT", nullable, unique)]
    title: String
}

#[tokio::test]
async fn connect_to_database() {
    let conn = connect("postgres://me:me@localhost:5432", NoTls)
        .await
        .expect("couln't connect to database");
    tokio::spawn(async move { conn.await.unwrap() });
}