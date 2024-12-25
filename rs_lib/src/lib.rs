// Copyright 2018-2024 the Deno authors. MIT license.

mod cache;
mod common;
mod deno_dir;
#[cfg(feature = "file_fetcher")]
pub mod file_fetcher;
mod global;
mod local;
pub mod memory;
pub mod npm;
mod sys;

/// Permissions used to save a file in the disk caches.
pub const CACHE_PERM: u32 = 0o644;

pub use cache::url_to_filename;
pub use cache::CacheEntry;
pub use cache::CacheReadFileError;
pub use cache::Checksum;
pub use cache::ChecksumIntegrityError;
pub use cache::GlobalToLocalCopy;
pub use cache::HttpCache;
pub use cache::HttpCacheItemKey;
pub use cache::SerializedCachedUrlMetadata;
pub use common::HeadersMap;
pub use deno_dir::resolve_deno_dir;
pub use global::GlobalHttpCache;
pub use local::LocalHttpCache;
pub use local::LocalLspHttpCache;
pub use sys::DenoCacheSys;

#[cfg(feature = "wasm")]
pub mod wasm {
  use std::collections::HashMap;
  use std::io::ErrorKind;
  use std::path::PathBuf;
  use std::sync::Arc;

  use js_sys::Object;
  use js_sys::Reflect;
  use js_sys::Uint8Array;
  use sys_traits::impls::wasm_path_to_str;
  use sys_traits::impls::wasm_string_to_path;
  use sys_traits::impls::RealSys;
  use url::Url;
  use wasm_bindgen::prelude::*;

  use crate::cache::CacheEntry;
  use crate::cache::GlobalToLocalCopy;
  use crate::common::HeadersMap;
  use crate::deno_dir;
  use crate::CacheReadFileError;
  use crate::Checksum;
  use crate::HttpCache;

  #[wasm_bindgen]
  pub fn url_to_filename(url: &str) -> Result<String, JsValue> {
    console_error_panic_hook::set_once();
    let url = parse_url(url).map_err(as_js_error)?;
    crate::cache::url_to_filename(&url)
      .map(|s| s.to_string_lossy().to_string())
      .map_err(as_js_error)
  }

  #[wasm_bindgen]
  pub fn resolve_deno_dir(
    maybe_custom_root: Option<String>,
  ) -> Result<String, JsValue> {
    console_error_panic_hook::set_once();
    deno_dir::resolve_deno_dir(
      &RealSys,
      maybe_custom_root.map(wasm_string_to_path),
    )
    .map(|path| wasm_path_to_str(&path).into_owned())
    .map_err(|e| JsValue::from(js_sys::Error::new(&e.to_string())))
  }

  #[wasm_bindgen]
  pub struct GlobalHttpCache {
    cache: crate::GlobalHttpCache<RealSys>,
  }

  #[wasm_bindgen]
  impl GlobalHttpCache {
    pub fn new(path: &str) -> Self {
      Self {
        cache: crate::GlobalHttpCache::new(RealSys, PathBuf::from(path)),
      }
    }

    #[wasm_bindgen(js_name = getHeaders)]
    pub fn get_headers(&self, url: &str) -> Result<JsValue, JsValue> {
      get_headers(&self.cache, url)
    }

    pub fn get(
      &self,
      url: &str,
      maybe_checksum: Option<String>,
    ) -> Result<JsValue, JsValue> {
      get_cache_entry(&self.cache, url, maybe_checksum.as_deref())
    }

    pub fn set(
      &self,
      url: &str,
      headers: JsValue,
      text: &[u8],
    ) -> Result<(), JsValue> {
      set(&self.cache, url, headers, text)
    }
  }

  #[wasm_bindgen]
  pub struct LocalHttpCache {
    cache: crate::LocalHttpCache<RealSys>,
  }

  #[wasm_bindgen]
  impl LocalHttpCache {
    pub fn new(
      local_path: String,
      global_path: String,
      allow_global_to_local_copy: bool,
    ) -> Self {
      console_error_panic_hook::set_once();
      let global =
        crate::GlobalHttpCache::new(RealSys, wasm_string_to_path(global_path));
      let local = crate::LocalHttpCache::new(
        wasm_string_to_path(local_path),
        Arc::new(global),
        if allow_global_to_local_copy {
          GlobalToLocalCopy::Allow
        } else {
          GlobalToLocalCopy::Disallow
        },
      );
      Self { cache: local }
    }

    #[wasm_bindgen(js_name = getHeaders)]
    pub fn get_headers(&self, url: &str) -> Result<JsValue, JsValue> {
      get_headers(&self.cache, url)
    }

    pub fn get(
      &self,
      url: &str,
      maybe_checksum: Option<String>,
    ) -> Result<JsValue, JsValue> {
      get_cache_entry(&self.cache, url, maybe_checksum.as_deref())
    }

    pub fn set(
      &self,
      url: &str,
      headers: JsValue,
      text: &[u8],
    ) -> Result<(), JsValue> {
      set(&self.cache, url, headers, text)
    }
  }

  fn get_headers<Cache: HttpCache>(
    cache: &Cache,
    url: &str,
  ) -> Result<JsValue, JsValue> {
    fn inner<Cache: HttpCache>(
      cache: &Cache,
      url: &str,
    ) -> std::io::Result<Option<HeadersMap>> {
      let url = parse_url(url)?;
      let key = cache.cache_item_key(&url)?;
      cache.read_headers(&key)
    }

    inner(cache, url)
      .map(|headers| match headers {
        Some(headers) => serde_wasm_bindgen::to_value(&headers).unwrap(),
        None => JsValue::undefined(),
      })
      .map_err(as_js_error)
  }

  fn get_cache_entry<Cache: HttpCache>(
    cache: &Cache,
    url: &str,
    maybe_checksum: Option<&str>,
  ) -> Result<JsValue, JsValue> {
    fn inner<Cache: HttpCache>(
      cache: &Cache,
      url: &str,
      maybe_checksum: Option<Checksum>,
    ) -> std::io::Result<Option<CacheEntry>> {
      let url = parse_url(url)?;
      let key = cache.cache_item_key(&url)?;
      match cache.get(&key, maybe_checksum) {
        Ok(Some(entry)) => Ok(Some(entry)),
        Ok(None) => Ok(None),
        Err(err) => match err {
          CacheReadFileError::Io(err) => Err(err),
          CacheReadFileError::ChecksumIntegrity(err) => {
            Err(std::io::Error::new(ErrorKind::InvalidData, err.to_string()))
          }
        },
      }
    }

    inner(cache, url, maybe_checksum.map(Checksum::new))
      .map(|text| match text {
        Some(entry) => {
          let content = {
            let array = Uint8Array::new_with_length(entry.content.len() as u32);
            array.copy_from(&entry.content);
            JsValue::from(array)
          };
          let headers: JsValue = {
            // make it an object instead of a Map
            let headers_object = Object::new();
            for (key, value) in &entry.metadata.headers {
              Reflect::set(
                &headers_object,
                &JsValue::from_str(key),
                &JsValue::from_str(value),
              )
              .unwrap();
            }
            JsValue::from(headers_object)
          };
          let obj = Object::new();
          Reflect::set(&obj, &JsValue::from_str("content"), &content).unwrap();
          Reflect::set(&obj, &JsValue::from_str("headers"), &headers).unwrap();
          JsValue::from(obj)
        }
        None => JsValue::undefined(),
      })
      .map_err(as_js_error)
  }

  fn set<Cache: HttpCache>(
    cache: &Cache,
    url: &str,
    headers: JsValue,
    content: &[u8],
  ) -> Result<(), JsValue> {
    fn inner<Cache: HttpCache>(
      cache: &Cache,
      url: &str,
      headers: JsValue,
      content: &[u8],
    ) -> std::io::Result<()> {
      let url = parse_url(url)?;
      let headers: HashMap<String, String> =
        serde_wasm_bindgen::from_value(headers).map_err(|err| {
          std::io::Error::new(ErrorKind::InvalidData, err.to_string())
        })?;
      cache.set(&url, headers, content)
    }

    inner(cache, url, headers, content).map_err(as_js_error)
  }

  fn parse_url(url: &str) -> std::io::Result<Url> {
    Url::parse(url)
      .map_err(|e| std::io::Error::new(ErrorKind::InvalidInput, e.to_string()))
  }

  fn as_js_error(e: std::io::Error) -> JsValue {
    JsValue::from(js_sys::Error::new(&e.to_string()))
  }
}
