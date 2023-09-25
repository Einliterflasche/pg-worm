use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use tokio::runtime::Runtime;

use pg_worm::prelude::*;

#[allow(dead_code)]
#[derive(Model)]
struct Book {
    #[column(primary_key, auto)]
    id: i64,
    title: String,
    author_id: i64,
}

#[allow(dead_code)]
#[derive(Model)]
struct Author {
    id: i64,
    title: String,
}

fn setup() {
    // Use the tokio runtime to complete the setup in an async block
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        Connection::build("postgres://postgres:postgres@localhost:5432")
            .connect()
            .await
            .expect("benchmark setup: failed to connect");

        force_create_table!(Book, Author)
            .await
            .expect("benchmark setup: failed to create tables");
    });
}

fn bench_main(criterion: &mut Criterion) {
    setup();

    criterion.bench_function("all-books", |bench| {
        bench
            .to_async(Runtime::new().expect("failed to create runtime"))
            .iter(|| async {
                Book::select().await.expect("failed to query");
            })
    });
}

criterion_group!{
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(Duration::from_millis(100_000));
    targets = bench_main
}   
criterion_main!(benches);
