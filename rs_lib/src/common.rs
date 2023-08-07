// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::path::Path;

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

pub trait DenoCacheFs: Send + Sync + std::fmt::Debug + Clone {
  fn read_file_bytes(&self, path: &Path) -> std::io::Result<Option<Vec<u8>>>;
  fn atomic_write_file(&self, path: &Path, bytes: &[u8]) -> std::io::Result<()>;
}

pub fn checksum(v: &[&[u8]]) -> String {
  use ring::digest::Context;
  use ring::digest::SHA256;

  let mut ctx = Context::new(&SHA256);
  for src in v {
    ctx.update(src.as_ref());
  }
  let digest = ctx.finish();
  let out: Vec<String> = digest
    .as_ref()
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect();
  out.join("")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_gen() {
    let actual = checksum(&[b"hello world"]);
    assert_eq!(
      actual,
      "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
  }
}
