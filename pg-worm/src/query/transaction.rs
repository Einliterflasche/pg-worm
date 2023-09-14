use std::{
    alloc::{alloc, dealloc, handle_alloc_error, Layout},
    ptr::drop_in_place,
};

use tokio_postgres::Transaction as PgTransaction;

use crate::{fetch_client, pool::Client as PgClient, Error};

use super::{Executable, Query, ToQuery};

struct PinnedClient(pub *mut PgClient);

impl PinnedClient {
    unsafe fn from_client(client: PgClient) -> PinnedClient {
        // Allocate memory on the heap
        let layout = Layout::new::<PgClient>();
        let pointer = alloc(layout) as *mut PgClient;

        // Make sure it worked
        if pointer.is_null() {
            handle_alloc_error(layout);
        }

        // Move the client object to the heap
        pointer.write(client);

        // Return a `PinnedClient` object with a pointer
        // to the underlying client.
        PinnedClient(pointer)
    }
}

impl Drop for PinnedClient {
    fn drop(&mut self) {
        unsafe {
            // Call `drop` on the client object to make sure
            // it is properly cleaned up and
            // returned to the pool.
            drop_in_place(self.0);

            // Deallocate the previously allocated
            // memory when the PinnedClient is dropped.
            dealloc(self.0 as *mut u8, Layout::new::<PgClient>());
        }
    }
}

/// A struct providing transaction functionality.
///
/// Use it to execute queries as part of this transaction.
/// When you are done, commit using `.commit()`
pub struct Transaction<'a> {
    transaction: PgTransaction<'a>,
    _client: PinnedClient,
}

impl<'a> Transaction<'a> {
    async fn from_client<'this>(client: PgClient) -> Result<Transaction<'a>, Error> {
        let client = unsafe { PinnedClient::from_client(client) };
        let transaction = unsafe {
            // Convert `*mut PgClient` to `&mut PgClient`
            // This shouldn't fail since the pointer in PinnedCliend
            // is guaranteed not to be null.
            &mut *client.0
        }
        .transaction()
        .await?;

        Ok(Transaction {
            _client: client,
            transaction,
        })
    }

    /// Begin a new transaction.
    pub async fn begin() -> Result<Transaction<'a>, Error> {
        let client = fetch_client().await?;

        Transaction::from_client(client).await
    }

    /// Rollback this transaction. TODO
    pub async fn rollback(self) -> Result<(), Error> {
        self.transaction.rollback().await.map_err(Error::from)
    }

    /// Commit the transaction. TODO
    pub async fn commit(self) -> Result<(), Error> {
        self.transaction.commit().await.map_err(Error::from)
    }

    /// Execute a query  as part of this transaction
    /// and return its return value.
    pub async fn execute<'b, Q, T>(&self, mut query: Q) -> Result<T, Error>
    where
        Q: ToQuery<'b, T>,
        Query<'b, T>: Executable<Output = T>,
    {
        let query = query.to_query();
        query.exec_with(&self.transaction).await
    }
}
