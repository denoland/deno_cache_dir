// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

mod cache;
mod common;
mod global;
mod local;

pub use cache::CachedUrlMetadata;
pub use cache::HttpCache;
pub use cache::HttpCacheItemKey;
pub use cache::SerializedCachedUrlMetadata;
pub use common::DenoCacheEnv;
pub use global::url_to_filename;
pub use global::GlobalHttpCache;
pub use local::LocalHttpCache;
pub use local::LocalLspHttpCache;

#[cfg(feature = "wasm")]
pub mod wasm {
  use std::collections::HashMap;
  use std::io::ErrorKind;
  use std::path::Path;
  use std::path::PathBuf;
  use std::sync::Arc;
  use std::time::SystemTime;

  use url::Url;
  use wasm_bindgen::prelude::*;

  use crate::CachedUrlMetadata;
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
    fn read_file_bytes(&self, path: &Path) -> std::io::Result<Option<Vec<u8>>> {
      let js_value =
        read_file_bytes(&path.to_string_lossy()).map_err(js_to_io_error)?;
      if js_value.is_null() || js_value.is_undefined() {
        Ok(None)
      } else {
        Ok(Some(js_sys::Uint8Array::from(js_value).to_vec()))
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
    let url = Url::parse(url).map_err(|e| as_js_error(e.into()))?;
    crate::global::url_to_filename(&url)
      .map(|s| s.to_string_lossy().to_string())
      .map_err(|e| as_js_error(e.into()))
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

    #[wasm_bindgen(js_name = getFileText)]
    pub fn get_file_text(&self, url: &str) -> Result<JsValue, JsValue> {
      get_file_text(&self.cache, url)
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
    pub fn new(local_path: &str, global_path: &str) -> Self {
      console_error_panic_hook::set_once();
      let global =
        crate::GlobalHttpCache::new(PathBuf::from(global_path), WasmEnv);
      let local =
        crate::LocalHttpCache::new(PathBuf::from(local_path), Arc::new(global));
      Self { cache: local }
    }

    #[wasm_bindgen(js_name = getHeaders)]
    pub fn get_headers(&self, url: &str) -> Result<JsValue, JsValue> {
      get_headers(&self.cache, url)
    }

    #[wasm_bindgen(js_name = getFileText)]
    pub fn get_file_text(&self, url: &str) -> Result<JsValue, JsValue> {
      get_file_text(&self.cache, url)
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
    ) -> anyhow::Result<Option<CachedUrlMetadata>> {
      let url = Url::parse(url)?;
      let key = cache.cache_item_key(&url)?;
      cache.read_metadata(&key)
    }

    inner(cache, url)
      .map(|metadata| match metadata {
        Some(metadata) => {
          serde_wasm_bindgen::to_value(&metadata.headers).unwrap()
        }
        None => JsValue::undefined(),
      })
      .map_err(as_js_error)
  }

  fn get_file_text<Cache: HttpCache>(
    cache: &Cache,
    url: &str,
  ) -> Result<JsValue, JsValue> {
    fn inner<Cache: HttpCache>(
      cache: &Cache,
      url: &str,
    ) -> anyhow::Result<Option<String>> {
      let url = Url::parse(url)?;
      let key = cache.cache_item_key(&url)?;
      match cache.read_file_bytes(&key)? {
        Some(bytes) => Ok(Some(String::from_utf8(bytes)?)),
        None => Ok(None),
      }
    }

    inner(cache, url)
      .map(|text| match text {
        Some(text) => JsValue::from(text),
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
    ) -> anyhow::Result<()> {
      let url = Url::parse(url)?;
      let headers: HashMap<String, String> =
        serde_wasm_bindgen::from_value(headers)
          .map_err(|err| anyhow::anyhow!("{}", err))?;
      cache.set(&url, headers, content)
    }

    inner(cache, url, headers, content).map_err(as_js_error)
  }

  fn as_js_error(e: anyhow::Error) -> JsValue {
    JsValue::from(js_sys::Error::new(&e.to_string()))
  }

  fn js_to_io_error(e: JsValue) -> std::io::Error {
    std::io::Error::new(ErrorKind::Other, format!("JS Error: {:?}", e))
  }
}
