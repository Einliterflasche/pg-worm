use futures_util::Future;
use pg_worm::{prelude::*, query::Prepared};
use tokio::time::Instant;

#[allow(dead_code)]
#[derive(Model, Debug)]
struct Book {
    #[column(primary_key, auto)]
    id: i64,
    title: String
}

pub trait Average {
    fn avg(&self) -> f64;
}

impl Average for Vec<u128> {
    fn avg(&self) -> f64 {
        (self.iter().sum::<u128>() as f64) / (self.len() as f64)
    }
}

async fn test_n_times_nanos<T>(n: usize, f: impl Fn(i64) -> T) -> Vec<u128> 
where
    T: Future
{
    let mut v = Vec::with_capacity(n);

    for i in 0..n {
        let f = f(i.try_into().unwrap());
        let before = Instant::now();
        f.await;
        let after = Instant::now();

        v.push(after.duration_since(before).as_nanos())
    }

    v
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Connection::build("postgres://postgres:postgres@localhost:5432")
        .connect()
        .await?;

    force_create_table!(Book).await?;

    Book::insert("Foo - Part I").await?;
    Book::insert("Foo - Part II").await?;
    Book::insert("Foo - Part III").await?;

    const N: usize = 1_000;
    let normal = test_n_times_nanos(N, |n| async move {
        Book::update()
            .set(Book::title, &format!("Foo {n}"))
            .where_(Book::id.lt(&n))
            .await.expect("err in book select");
    }).await;
    let prepared = test_n_times_nanos(N, |n| async move {
        Book::update()
            .set(Book::title, &format!("Foo {n}"))
            .where_(Book::id.lt(&n))
            .prepared()
            .await.expect("err in book select");
    }).await;
    
    println!("normal avg:    {}µs\nprepared avg:  {}µs", normal.avg() / 1000f64, prepared.avg() / 1000f64);

    Ok(())
}