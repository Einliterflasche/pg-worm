use std::{ptr::drop_in_place, alloc::{dealloc, Layout, alloc, handle_alloc_error}};

use deadpool_postgres::{Client as DpClient, Transaction as DpTransaction};

use crate::{fetch_client, Error};

use super::{ToQuery, Query, Executable};


struct PinnedClient(pub *mut DpClient);

impl PinnedClient {
    unsafe fn from_client(client: DpClient) -> PinnedClient {
        // Allocate memory on the heap
        let layout = Layout::new::<DpClient>();
        let pointer = alloc(layout) as *mut DpClient;

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
            dealloc(self.0 as *mut u8, Layout::new::<DpClient>());
        }
    }
}


/// A struct providing transaction functionality. 
/// 
/// Use it to execute queries as part of this transaction.
/// When you are done, commit using `.commit()`
pub struct Transaction<'a> {
    transaction: DpTransaction<'a>,
    _client: PinnedClient
}

impl<'a> Transaction<'a> {
    async fn from_client<'this>(client: DpClient) -> Result<Transaction<'a>, Error> {
        let client = unsafe { PinnedClient::from_client(client) };
        let transaction = unsafe {
            // Convert `*mut DpClient` to `&mut DpClient`
            // This shouldn't fail since the pointer in PinnedCliend
            // is guaranteed not to be null.
            &mut *client.0
        }.transaction().await?;

        Ok(
            Transaction { _client: client, transaction }
        )
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