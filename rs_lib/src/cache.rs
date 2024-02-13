// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use anyhow::Error as AnyError;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;
use std::time::SystemTime;
use url::Url;

use crate::common::HeadersMap;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SerializedCachedUrlMetadata {
  pub headers: HeadersMap,
  pub url: String,
  #[serde(rename = "now")]
  pub time: Option<SystemTime>,
}

/// Computed cache key, which can help reduce the work of computing the cache key multiple times.
pub struct HttpCacheItemKey<'a> {
  // The key is specific to the implementation of HttpCache,
  // so keep these private to the module. For example, the
  // fact that these may be stored in a file is an implementation
  // detail.
  #[cfg(debug_assertions)]
  pub(super) is_local_key: bool,
  pub(super) url: &'a Url,
  /// This will be set all the time for the global cache, but it
  /// won't ever be set for the local cache because that also needs
  /// header information to determine the final path.
  pub(super) file_path: Option<PathBuf>,
}

pub trait HttpCache: Send + Sync + std::fmt::Debug {
  /// A pre-computed key for looking up items in the cache.
  fn cache_item_key<'a>(
    &self,
    url: &'a Url,
  ) -> Result<HttpCacheItemKey<'a>, AnyError>;

  fn contains(&self, url: &Url) -> bool;
  fn set(
    &self,
    url: &Url,
    headers: HeadersMap,
    content: &[u8],
  ) -> Result<(), AnyError>;
  fn read_modified_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<SystemTime>, AnyError>;
  fn read_file_bytes(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<Vec<u8>>, AnyError>;
  /// Reads the headers for the cache item.
  fn read_headers(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<HeadersMap>, AnyError>;
  /// Reads the time the item was downloaded to the cache.
  fn read_download_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<SystemTime>, AnyError>;
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn deserialized_no_now() {
    let json = r#"{
      "headers": {
        "content-type": "application/javascript"
      },
      "url": "https://deno.land/std/http/file_server.ts"
    }"#;
    let data: SerializedCachedUrlMetadata = serde_json::from_str(json).unwrap();
    assert_eq!(
      data,
      SerializedCachedUrlMetadata {
        headers: HeadersMap::from([(
          "content-type".to_string(),
          "application/javascript".to_string()
        )]),
        time: None,
        url: "https://deno.land/std/http/file_server.ts".to_string(),
      }
    );
  }
}
