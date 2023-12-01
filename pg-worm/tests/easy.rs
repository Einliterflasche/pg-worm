use pg_worm::Model;

#[derive(Model)]
struct Bar {
    #[column(name = "_id", primary_key)]
    id: i64,
}

impl Bar {
    async fn foo() {
        Bar::query("", vec![]).await;
    }
}
