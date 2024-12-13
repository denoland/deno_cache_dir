// Copyright 2018-2024 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use parking_lot::Mutex;
use url::Url;

use crate::CacheEntry;
use crate::CacheReadFileError;
use crate::Checksum;
use crate::HeadersMap;
use crate::HttpCache;
use crate::HttpCacheItemKey;
use crate::SerializedCachedUrlMetadata;

pub trait MemoryHttpCacheClock: std::fmt::Debug + Send + Sync {
  fn time_now(&self) -> SystemTime;
}

#[derive(Debug)]
pub struct MemoryHttpCacheSystemTimeClock;

impl MemoryHttpCacheClock for MemoryHttpCacheSystemTimeClock {
  fn time_now(&self) -> SystemTime {
    #[allow(clippy::disallowed_methods)]
    SystemTime::now()
  }
}

/// A simple in-memory cache mostly useful for testing.
#[derive(Debug)]
pub struct MemoryHttpCache {
  cache: Mutex<HashMap<Url, CacheEntry>>,
  clock: Arc<dyn MemoryHttpCacheClock>,
}

impl Default for MemoryHttpCache {
  fn default() -> Self {
    Self::new(Arc::new(MemoryHttpCacheSystemTimeClock))
  }
}

impl MemoryHttpCache {
  pub fn new(clock: Arc<dyn MemoryHttpCacheClock>) -> Self {
    Self {
      cache: Mutex::new(HashMap::new()),
      clock,
    }
  }
}

impl HttpCache for MemoryHttpCache {
  fn cache_item_key<'a>(
    &self,
    url: &'a Url,
  ) -> std::io::Result<HttpCacheItemKey<'a>> {
    Ok(HttpCacheItemKey {
      #[cfg(debug_assertions)]
      is_local_key: false,
      url,
      file_path: None,
    })
  }

  fn contains(&self, url: &Url) -> bool {
    self.cache.lock().contains_key(url)
  }

  fn set(
    &self,
    url: &Url,
    headers: HeadersMap,
    content: &[u8],
  ) -> std::io::Result<()> {
    self.cache.lock().insert(
      url.clone(),
      CacheEntry {
        metadata: SerializedCachedUrlMetadata {
          headers,
          url: url.to_string(),
          time: Some(
            self
              .clock
              .time_now()
              .duration_since(UNIX_EPOCH)
              .unwrap()
              .as_secs(),
          ),
        },
        content: Cow::Owned(content.to_vec()),
      },
    );
    Ok(())
  }

  fn get(
    &self,
    key: &HttpCacheItemKey,
    maybe_checksum: Option<Checksum>,
  ) -> Result<Option<CacheEntry>, CacheReadFileError> {
    self
      .cache
      .lock()
      .get(key.url)
      .cloned()
      .map(|entry| {
        if let Some(checksum) = maybe_checksum {
          checksum
            .check(key.url, &entry.content)
            .map_err(CacheReadFileError::ChecksumIntegrity)?;
        }
        Ok(entry)
      })
      .transpose()
  }

  fn read_modified_time(
    &self,
    _key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<std::time::SystemTime>> {
    Ok(None) // for now
  }

  fn read_headers(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<HeadersMap>> {
    Ok(
      self
        .cache
        .lock()
        .get(key.url)
        .map(|entry| entry.metadata.headers.clone()),
    )
  }

  fn read_download_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<std::time::SystemTime>> {
    Ok(self.cache.lock().get(key.url).and_then(|entry| {
      entry
        .metadata
        .time
        .map(|time| UNIX_EPOCH + std::time::Duration::from_secs(time))
    }))
  }
}
