use pg_worm_derive::Model;

#[derive(Model)]
struct Foo {
    id: i64,
}

#[derive(Model)]
struct Bar {
    #[column(name = "_id", primary_key)]
    id: i64,
}
