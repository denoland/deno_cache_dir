
mod cache;
mod common;
mod global;
mod local;

pub use global::GlobalHttpCache;
pub use global::url_to_filename;
pub use common::DenoCacheFs;
pub use cache::HttpCache;
pub use cache::CachedUrlMetadata;
pub use cache::HttpCacheItemKey;
pub use local::LocalHttpCache;
pub use local::LocalLspHttpCache;

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub fn add(a: i32, b: i32) -> i32 {
  a + b
}
