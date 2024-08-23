// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

mod cache;
mod common;
mod env;
mod global;
mod local;

pub use cache::url_to_filename;
pub use cache::CacheEntry;
pub use cache::CacheReadFileError;
pub use cache::Checksum;
pub use cache::ChecksumIntegrityError;
pub use cache::GlobalToLocalCopy;
pub use cache::HttpCache;
pub use cache::HttpCacheItemKey;
pub use cache::HttpCacheItemKeyDestination;
pub use cache::SerializedCachedUrlMetadata;
pub use env::DenoCacheEnv;
pub use global::GlobalHttpCache;
pub use local::LocalHttpCache;
pub use local::LocalLspHttpCache;

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub use env::TestRealDenoCacheEnv;

#[cfg(feature = "wasm")]
pub mod wasm {
  use std::collections::HashMap;
  use std::io::ErrorKind;
  use std::path::Path;
  use std::path::PathBuf;
  use std::sync::Arc;
  use std::time::SystemTime;

  use js_sys::Object;
  use js_sys::Reflect;
  use js_sys::Uint8Array;
  use url::Url;
  use wasm_bindgen::prelude::*;

  use crate::cache::CacheEntry;
  use crate::cache::GlobalToLocalCopy;
  use crate::common::HeadersMap;
  use crate::CacheReadFileError;
  use crate::Checksum;
  use crate::DenoCacheEnv;
  use crate::HttpCache;

  #[wasm_bindgen(module = "/fs.js")]
  extern "C" {
    #[wasm_bindgen(catch)]
    fn read_file_bytes(path: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch)]
    fn atomic_write_file(path: &str, bytes: &[u8]) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch)]
    fn modified_time(path: &str) -> Result<Option<usize>, JsValue>;
    fn is_file(path: &str) -> bool;
    fn time_now() -> usize;
  }

  #[derive(Clone, Debug)]
  struct WasmEnv;

  impl DenoCacheEnv for WasmEnv {
    fn read_file_bytes(&self, path: &Path) -> std::io::Result<Vec<u8>> {
      let js_value =
        read_file_bytes(&path.to_string_lossy()).map_err(js_to_io_error)?;
      if js_value.is_null() || js_value.is_undefined() {
        Err(std::io::Error::new(ErrorKind::NotFound, ""))
      } else {
        Ok(js_sys::Uint8Array::from(js_value).to_vec())
      }
    }

    fn atomic_write_file(
      &self,
      path: &Path,
      bytes: &[u8],
    ) -> std::io::Result<()> {
      atomic_write_file(&path.to_string_lossy(), bytes)
        .map_err(js_to_io_error)?;
      Ok(())
    }

    fn modified(&self, path: &Path) -> std::io::Result<Option<SystemTime>> {
      if let Some(time) =
        modified_time(&path.to_string_lossy()).map_err(js_to_io_error)?
      {
        Ok(Some(
          SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(time as u64),
        ))
      } else {
        Ok(None)
      }
    }

    fn is_file(&self, path: &Path) -> bool {
      is_file(&path.to_string_lossy())
    }

    fn time_now(&self) -> SystemTime {
      let time = time_now();
      SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(time as u64)
    }
  }

  #[wasm_bindgen]
  pub fn url_to_filename(url: &str) -> Result<String, JsValue> {
    console_error_panic_hook::set_once();
    let url = parse_url(url).map_err(as_js_error)?;
    crate::cache::url_to_filename(&url)
      .map(|s| s.to_string_lossy().to_string())
      .map_err(as_js_error)
  }

  #[wasm_bindgen]
  pub struct GlobalHttpCache {
    cache: crate::GlobalHttpCache<WasmEnv>,
  }

  #[wasm_bindgen]
  impl GlobalHttpCache {
    pub fn new(path: &str) -> Self {
      Self {
        cache: crate::GlobalHttpCache::new(PathBuf::from(path), WasmEnv),
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
    cache: crate::LocalHttpCache<WasmEnv>,
  }

  #[wasm_bindgen]
  impl LocalHttpCache {
    pub fn new(
      local_path: &str,
      global_path: &str,
      allow_global_to_local_copy: bool,
    ) -> Self {
      console_error_panic_hook::set_once();
      let global =
        crate::GlobalHttpCache::new(PathBuf::from(global_path), WasmEnv);
      let local = crate::LocalHttpCache::new(
        PathBuf::from(local_path),
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

  fn js_to_io_error(e: JsValue) -> std::io::Error {
    std::io::Error::new(ErrorKind::Other, format!("JS Error: {:?}", e))
  }
}
