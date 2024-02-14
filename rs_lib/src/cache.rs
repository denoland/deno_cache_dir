// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use anyhow::Error as AnyError;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;
use std::time::SystemTime;
use thiserror::Error;
use url::Url;

use crate::common::base_url_to_filename_parts;
use crate::common::checksum;
use crate::common::HeadersMap;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum GlobalToLocalCopy {
  /// When using a local cache (vendor folder), allow the cache to
  /// copy from the global cache into the local one.
  Allow,
  /// Disallow copying from the global to the local cache. This is
  /// useful for the LSP because we want to ensure that checksums
  /// are evaluated for JSR dependencies, which is difficult to do
  /// in the LSP. This could be improved in the future to not require
  /// this
  Disallow,
}

impl GlobalToLocalCopy {
  pub fn is_true(&self) -> bool {
    matches!(self, GlobalToLocalCopy::Allow)
  }
}

#[derive(Debug, Error)]
#[error("Integrity check failed for: {}\n\nActual: {}\nExpected: {}", .url, .actual, .expected)]
pub struct ChecksumIntegrityError {
  pub url: Url,
  pub actual: String,
  pub expected: String,
}

#[derive(Debug, Clone, Copy)]
pub struct Checksum<'a>(&'a str);

impl<'a> Checksum<'a> {
  pub fn new(checksum: &'a str) -> Self {
    Self(checksum)
  }

  pub fn as_str(&self) -> &str {
    self.0
  }
}

#[derive(Debug, Error)]
#[error("Can't convert url (\"{}\") to filename.", .url)]
pub struct UrlToFilenameConversionError {
  pub(super) url: String,
}

/// Turn provided `url` into a hashed filename.
/// URLs can contain a lot of characters that cannot be used
/// in filenames (like "?", "#", ":"), so in order to cache
/// them properly they are deterministically hashed into ASCII
/// strings.
pub fn url_to_filename(
  url: &Url,
) -> Result<PathBuf, UrlToFilenameConversionError> {
  let Some(mut cache_filename) = base_url_to_filename(url) else {
    return Err(UrlToFilenameConversionError {
      url: url.to_string(),
    });
  };

  let mut rest_str = url.path().to_string();
  if let Some(query) = url.query() {
    rest_str.push('?');
    rest_str.push_str(query);
  }
  // NOTE: fragment is omitted on purpose - it's not taken into
  // account when caching - it denotes parts of webpage, which
  // in case of static resources doesn't make much sense
  let hashed_filename = checksum(rest_str.as_bytes());
  cache_filename.push(hashed_filename);
  Ok(cache_filename)
}

// Turn base of url (scheme, hostname, port) into a valid filename.
/// This method replaces port part with a special string token (because
/// ":" cannot be used in filename on some platforms).
/// Ex: $DENO_DIR/deps/https/deno.land/
fn base_url_to_filename(url: &Url) -> Option<PathBuf> {
  base_url_to_filename_parts(url, "_PORT").map(|parts| {
    let mut out = PathBuf::new();
    for part in parts {
      out.push(part);
    }
    out
  })
}

#[derive(Debug, Error)]
pub enum CacheReadFileError {
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[error(transparent)]
  UrlToFileName(#[from] UrlToFilenameConversionError),
  #[error(transparent)]
  ChecksumIntegrity(Box<ChecksumIntegrityError>),
  #[error(transparent)]
  ReadHeaders(anyhow::Error),
}

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
  ) -> Result<HttpCacheItemKey<'a>, UrlToFilenameConversionError>;

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
    maybe_checksum: Option<Checksum>,
    allow_global_to_local: GlobalToLocalCopy,
  ) -> Result<Option<Vec<u8>>, CacheReadFileError>;
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
