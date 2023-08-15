//! 

use std::str::FromStr;

use deadpool_postgres::{ManagerConfig, RecyclingMethod, Pool, Manager};
use tokio_postgres::{Config, NoTls, tls::{MakeTlsConnect, TlsConnect}, Socket};

use crate::Error;

/// 
pub struct Connection;

impl Connection {
    /// Connect `pg_worm` to postgres using the specified connection string
    /// and tls.
    pub async fn to_tls<T>(connection_string: impl Into<String>, tls: T) -> Result<(), Error>
    where 
        T: MakeTlsConnect<Socket> + Clone + Send + Sync + 'static,
        T::Stream: Sync + Send,
        T::TlsConnect: Sync + Send,
        <T::TlsConnect as TlsConnect<Socket>>::Future: Send
    {        
        let config = Config::from_str(connection_string.into().as_str())?;
        let manager_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast
        };

        let manager = Manager::from_config(config, tls, manager_config);

        let pool = Pool::builder(manager).max_size(4).build()?;

        crate::set_pool(pool)
    }

    /// Connect to a postgres server without using TLS 
    /// (only recommended for local databases).
    pub async fn to(connection_string: impl Into<String>) -> Result<(), Error> {
        Self::to_tls(connection_string, NoTls).await
    }
}
