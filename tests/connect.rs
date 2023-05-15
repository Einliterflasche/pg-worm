use pg_worm::tokio_postgres::NoTls;
use pg_worm::{connect, Model};

#[derive(Model)]
struct Book {
    #[column(dtype = "BIGSERIAL", primary_key, unique)]
    id: i64,
    #[column(dtype = "TEXT", unique)]
    title: String,
}

#[tokio::test]
async fn connect_to_database() {
    let conn = connect("postgres://me:me@localhost:5432", NoTls)
        .await
        .expect("couln't connect to database");
    tokio::spawn(async move { conn.await.unwrap() });

    pg_worm::register!(Book).await.unwrap();
}
