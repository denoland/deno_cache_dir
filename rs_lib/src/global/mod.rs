// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use url::Url;

use super::cache::HttpCache;
use super::cache::HttpCacheItemKey;
use super::env::DenoCacheEnv;
use crate::cache::url_to_filename;
use crate::cache::CacheEntry;
use crate::cache::CacheReadFileError;
use crate::cache::Checksum;
use crate::cache::HttpCacheItemKeyDestination;
use crate::cache::SerializedCachedUrlMetadata;
use crate::common::checksum;
use crate::common::HeadersMap;
use crate::ChecksumIntegrityError;

mod cache_file;

#[derive(Debug)]
pub struct GlobalHttpCache<Env: DenoCacheEnv> {
  path: PathBuf,
  pub(crate) env: Env,
}

impl<Env: DenoCacheEnv> GlobalHttpCache<Env> {
  pub fn new(path: PathBuf, env: Env) -> Self {
    #[cfg(not(feature = "wasm"))]
    assert!(path.is_absolute());
    Self { path, env }
  }

  pub fn get_global_cache_location(&self) -> &PathBuf {
    &self.path
  }

  pub fn get_global_cache_filepath(
    &self,
    url: &Url,
    destination: HttpCacheItemKeyDestination,
  ) -> std::io::Result<PathBuf> {
    Ok(self.path.join(url_to_filename(url, destination)?))
  }

  fn get_cache_filepath(
    &self,
    url: &Url,
    destination: HttpCacheItemKeyDestination,
  ) -> std::io::Result<PathBuf> {
    Ok(self.path.join(url_to_filename(url, destination)?))
  }

  #[inline]
  fn key_file_path<'a>(&self, key: &'a HttpCacheItemKey) -> &'a PathBuf {
    // The key file path is always set for the global cache because
    // the file will always exist, unlike the local cache, which won't
    // have this for redirects.
    key.file_path.as_ref().unwrap()
  }
}

impl<Env: DenoCacheEnv> HttpCache for GlobalHttpCache<Env> {
  fn cache_item_key<'a>(
    &self,
    url: &'a Url,
    destination: HttpCacheItemKeyDestination,
  ) -> std::io::Result<HttpCacheItemKey<'a>> {
    Ok(HttpCacheItemKey {
      #[cfg(debug_assertions)]
      is_local_key: false,
      url,
      destination,
      file_path: Some(self.get_cache_filepath(url, destination)?),
    })
  }

  fn contains(
    &self,
    url: &Url,
    destination: HttpCacheItemKeyDestination,
  ) -> bool {
    let Ok(cache_filepath) = self.get_cache_filepath(url, destination) else {
      return false;
    };
    self.env.is_file(&cache_filepath)
  }

  fn read_modified_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<SystemTime>> {
    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);

    self.env.modified(self.key_file_path(key))
  }

  fn set(
    &self,
    url: &Url,
    destination: HttpCacheItemKeyDestination,
    headers: HeadersMap,
    content: &[u8],
  ) -> std::io::Result<()> {
    let cache_filepath = self.get_cache_filepath(url, destination)?;
    cache_file::write(
      &self.env,
      &cache_filepath,
      content,
      &SerializedCachedUrlMetadata {
        time: Some(
          self
            .env
            .time_now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        ),
        url: url.to_string(),
        headers,
      },
    )
    .unwrap();

    Ok(())
  }

  fn get(
    &self,
    key: &HttpCacheItemKey,
    maybe_checksum: Option<Checksum>,
  ) -> Result<Option<CacheEntry>, CacheReadFileError> {
    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);

    let maybe_file = cache_file::read(&self.env, self.key_file_path(key))?;

    if let Some(file) = &maybe_file {
      if let Some(expected_checksum) = maybe_checksum {
        let actual = checksum(&file.content);
        if expected_checksum.as_str() != actual {
          return Err(CacheReadFileError::ChecksumIntegrity(Box::new(
            ChecksumIntegrityError {
              url: key.url.clone(),
              expected: expected_checksum.as_str().to_string(),
              actual,
            },
          )));
        }
      }
    }

    Ok(maybe_file)
  }

  fn read_headers(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<HeadersMap>> {
    // targeted deserialize
    #[derive(Deserialize)]
    struct SerializedHeaders {
      pub headers: HeadersMap,
    }

    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);

    let maybe_metadata = cache_file::read_metadata::<SerializedHeaders>(
      &self.env,
      self.key_file_path(key),
    )?;
    Ok(maybe_metadata.map(|m| m.headers))
  }

  fn read_download_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<SystemTime>> {
    // targeted deserialize
    #[derive(Deserialize)]
    struct SerializedTime {
      pub time: Option<u64>,
    }

    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);
    let maybe_metadata = cache_file::read_metadata::<SerializedTime>(
      &self.env,
      self.key_file_path(key),
    )?;
    Ok(maybe_metadata.and_then(|m| {
      Some(SystemTime::UNIX_EPOCH + Duration::from_secs(m.time?))
    }))
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_url_to_filename() {
    run_test(
      "https://deno.land/x/foo.ts",
      HttpCacheItemKeyDestination::Script,
      "https/deno.land/2c0a064891b9e3fbe386f5d4a833bce5076543f5404613656042107213a7bbc8"
    );
    run_test(
      "https://deno.land:8080/x/foo.ts",
      HttpCacheItemKeyDestination::Script,
      "https/deno.land_PORT8080/2c0a064891b9e3fbe386f5d4a833bce5076543f5404613656042107213a7bbc8",
    );
    run_test(
      "https://deno.land/",
      HttpCacheItemKeyDestination::Script,
      "https/deno.land/8a5edab282632443219e051e4ade2d1d5bbc671c781051bf1437897cbdfea0f1",
    );
    run_test(
      "https://deno.land/?asdf=qwer",
      HttpCacheItemKeyDestination::Script,
      "https/deno.land/e4edd1f433165141015db6a823094e6bd8f24dd16fe33f2abd99d34a0a21a3c0",
    );
    // should be the same as case above, fragment (#qwer) is ignored
    // when hashing
    run_test(
      "https://deno.land/?asdf=qwer#qwer",
      HttpCacheItemKeyDestination::Script,
      "https/deno.land/e4edd1f433165141015db6a823094e6bd8f24dd16fe33f2abd99d34a0a21a3c0",
    );
    run_test(
      "https://deno.land/data.json",
       HttpCacheItemKeyDestination::Json,
      "https\\deno.land\\ca2c34679b71e39cd6c440a4fa4e7e3add3c96040571a12b34a8683eff28e410",
    );
    run_test(
      "https://deno.land/data.json",
      // now try with script
      HttpCacheItemKeyDestination::Script,
      "https\\deno.land\\1d010d39e2f8999e7b9c0abef8f1f92b572fa5868b8819355a8f489190f0d23b",
    );
    run_test(
      "data:application/typescript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=",
      HttpCacheItemKeyDestination::Script,
      "data/c21c7fc382b2b0553dc0864aa81a3acacfb7b3d1285ab5ae76da6abec213fb37",
    );
    run_test(
      "data:text/plain,Hello%2C%20Deno!",
      HttpCacheItemKeyDestination::Script,
      "data/967374e3561d6741234131e342bf5c6848b70b13758adfe23ee1a813a8131818",
    );

    #[track_caller]
    fn run_test(
      url: &str,
      destination: HttpCacheItemKeyDestination,
      expected: &str,
    ) {
      let u = Url::parse(url).unwrap();
      let p = url_to_filename(&u, destination).unwrap();
      assert_eq!(p, PathBuf::from(expected));
    }
  }
}
