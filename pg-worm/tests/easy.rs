use pg_worm::Model;

#[derive(Model)]
struct Bar {
    #[column(name = "_id", primary_key)]
    id: i64,
}

impl Bar {
    async fn foo() -> Result<(), pg_worm::Error> {
        Bar::query("", vec![]).await.map(|_| ())
    }
}
