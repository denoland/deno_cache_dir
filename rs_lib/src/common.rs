// Copyright 2018-2024 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;

use url::Url;

pub type HeadersMap = HashMap<String, String>;

pub fn base_url_to_filename_parts<'a>(
  url: &'a Url,
  port_separator: &str,
) -> Option<Vec<Cow<'a, str>>> {
  let mut out = Vec::with_capacity(2);

  let scheme = url.scheme();

  match scheme {
    "http" | "https" => {
      out.push(Cow::Borrowed(scheme));

      let host = url.host_str().unwrap();
      let host_port = match url.port() {
        // underscores are not allowed in domains, so adding one here is fine
        Some(port) => Cow::Owned(format!("{host}{port_separator}{port}")),
        None => Cow::Borrowed(host),
      };
      out.push(host_port);
    }
    "data" | "blob" => {
      out.push(Cow::Borrowed(scheme));
    }
    scheme => {
      log::debug!("Don't know how to create cache name for scheme: {}", scheme);
      return None;
    }
  };

  Some(out)
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
