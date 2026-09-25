//! The Databases workspace: saved connections, live sessions against them,
//! and the SQL Reach generates on the user's behalf.

pub mod backup;
pub mod conn;
pub mod csv;
pub mod design;
pub mod detect;
pub mod dialect;
pub mod guard;
pub mod introspect;
#[cfg(test)]
mod live_tests;
pub mod redis;
pub mod store;
pub mod types;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use conn::LiveDb;
use redis::LiveRedis;
use store::DbConnectionStore;

#[derive(Clone)]
pub enum Live {
    Sql(Arc<LiveDb>),
    Redis(Arc<LiveRedis>),
}

/// Saved connections, open ones, and running backup/restore/import jobs.
/// The lock is held only to look things up; work happens on the `Arc`s.
pub struct DbManager {
    pub store: DbConnectionStore,
    live: HashMap<String, Live>,
    jobs: HashMap<String, Arc<AtomicBool>>,
}

impl DbManager {
    pub fn new() -> Self {
        Self { store: DbConnectionStore::new(), live: HashMap::new(), jobs: HashMap::new() }
    }

    pub fn sql(&self, id: &str) -> Result<Arc<LiveDb>, String> {
        match self.live.get(id) {
            Some(Live::Sql(l)) => Ok(l.clone()),
            Some(Live::Redis(_)) => Err("That is a Redis connection".into()),
            None => Err("Not connected".into()),
        }
    }

    pub fn redis(&self, id: &str) -> Result<Arc<LiveRedis>, String> {
        match self.live.get(id) {
            Some(Live::Redis(l)) => Ok(l.clone()),
            Some(Live::Sql(_)) => Err("That is not a Redis connection".into()),
            None => Err("Not connected".into()),
        }
    }

    pub fn insert(&mut self, id: String, live: Live) -> Option<Live> {
        self.live.insert(id, live)
    }

    pub fn remove(&mut self, id: &str) -> Option<Live> {
        self.live.remove(id)
    }

    /// Close every open connection and stop every job.
    pub async fn close_all(&mut self) {
        for (_, live) in self.live.drain() {
            if let Live::Sql(l) = live {
                l.close().await;
            }
        }
        for flag in self.jobs.values() {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    pub fn start_job(&mut self, id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.jobs.insert(id.to_string(), flag.clone());
        flag
    }

    pub fn job(&self, id: &str) -> Option<Arc<AtomicBool>> {
        self.jobs.get(id).cloned()
    }

    pub fn end_job(&mut self, id: &str) {
        self.jobs.remove(id);
    }
}
