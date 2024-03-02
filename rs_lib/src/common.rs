// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

use url::Url;

pub type HeadersMap = HashMap<String, String>;

pub fn base_url_to_filename_parts(
  url: &Url,
  port_separator: &str,
) -> Option<Vec<String>> {
  let mut out = Vec::with_capacity(2);

  let scheme = url.scheme();
  out.push(scheme.to_string());

  match scheme {
    "http" | "https" => {
      let host = url.host_str().unwrap();
      let host_port = match url.port() {
        // underscores are not allowed in domains, so adding one here is fine
        Some(port) => format!("{host}{port_separator}{port}"),
        None => host.to_string(),
      };
      out.push(host_port);
    }
    "data" | "blob" => (),
    scheme => {
      log::debug!("Don't know how to create cache name for scheme: {}", scheme);
      return None;
    }
  };

  Some(out)
}

pub trait DenoCacheEnv: Send + Sync + std::fmt::Debug + Clone {
  fn read_file_bytes(&self, path: &Path) -> std::io::Result<Option<Vec<u8>>>;
  fn atomic_write_file(&self, path: &Path, bytes: &[u8])
    -> std::io::Result<()>;
  fn modified(&self, path: &Path) -> std::io::Result<Option<SystemTime>>;
  fn is_file(&self, path: &Path) -> bool;
  fn time_now(&self) -> SystemTime;
}

pub fn checksum(v: &[u8]) -> String {
  use sha2::Digest;
  use sha2::Sha256;

  let mut hasher = Sha256::new();
  hasher.update(v);
  format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_gen() {
    let actual = checksum(b"hello world");
    assert_eq!(
      actual,
      "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
  }
}
