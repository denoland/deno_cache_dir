// Copyright 2018-2024 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;

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

pub fn url_from_directory_path(path: &Path) -> Result<Url, ()> {
  #[cfg(any(unix, windows, target_os = "redox", target_os = "wasi"))]
  return Url::from_directory_path(path);
  #[cfg(not(any(unix, windows, target_os = "redox", target_os = "wasi")))]
  url_from_directory_path_wasm(path)
}

#[cfg(any(
  test,
  not(any(unix, windows, target_os = "redox", target_os = "wasi"))
))]
fn url_from_directory_path_wasm(path: &Path) -> Result<Url, ()> {
  let mut url = url_from_file_path_wasm(path)?;
  url.path_segments_mut().unwrap().push("");
  Ok(url)
}

#[cfg(any(
  test,
  not(any(unix, windows, target_os = "redox", target_os = "wasi"))
))]
fn url_from_file_path_wasm(path: &Path) -> Result<Url, ()> {
  use std::path::Component;

  let original_path = path.to_string_lossy();
  let mut path_str = original_path;
  // assume paths containing backslashes are windows paths
  if path_str.contains('\\') {
    let mut url = Url::parse("file://").unwrap();
    if let Some(next) = path_str.strip_prefix(r#"\\?\UNC\"#) {
      if let Some((host, rest)) = next.split_once('\\') {
        if url.set_host(Some(host)).is_ok() {
          path_str = rest.to_string().into();
        }
      }
    } else if let Some(next) = path_str.strip_prefix(r#"\\?\"#) {
      path_str = next.to_string().into();
    } else if let Some(next) = path_str.strip_prefix(r#"\\"#) {
      if let Some((host, rest)) = next.split_once('\\') {
        if url.set_host(Some(host)).is_ok() {
          path_str = rest.to_string().into();
        }
      }
    }

    for component in path_str.split('\\') {
      url.path_segments_mut().unwrap().push(component);
    }

    Ok(url)
  } else {
    let mut url = Url::parse("file://").unwrap();
    for component in path.components() {
      match component {
        Component::RootDir => {
          url.path_segments_mut().unwrap().push("");
        }
        Component::Normal(segment) => {
          url
            .path_segments_mut()
            .unwrap()
            .push(&segment.to_string_lossy());
        }
        Component::Prefix(_) | Component::CurDir | Component::ParentDir => {
          return Err(());
        }
      }
    }

    Ok(url)
  }
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

  #[test]
  fn test_url_from_file_path_wasm() {
    #[track_caller]
    fn convert(path: &str) -> String {
      url_from_file_path_wasm(Path::new(path))
        .unwrap()
        .to_string()
    }

    assert_eq!(convert("/a/b/c.json"), "file:///a/b/c.json");
    assert_eq!(
      convert("D:\\test\\other.json"),
      "file:///D:/test/other.json"
    );
    assert_eq!(
      convert("/path with spaces/and#special%chars!.json"),
      "file:///path%20with%20spaces/and%23special%25chars!.json"
    );
    assert_eq!(
      convert("C:\\My Documents\\file.txt"),
      "file:///C:/My%20Documents/file.txt"
    );
    assert_eq!(
      convert("/a/b/пример.txt"),
      "file:///a/b/%D0%BF%D1%80%D0%B8%D0%BC%D0%B5%D1%80.txt"
    );
    assert_eq!(
      convert("\\\\server\\share\\folder\\file.txt"),
      "file://server/share/folder/file.txt"
    );
    assert_eq!(convert(r#"\\?\UNC\server\share"#), "file://server/share");
    assert_eq!(
      convert(r"\\?\cat_pics\subfolder\file.jpg"),
      "file:///cat_pics/subfolder/file.jpg"
    );
    assert_eq!(convert(r"\\?\cat_pics"), "file:///cat_pics");
  }

  #[test]
  fn test_url_from_directory_path_wasm() {
    #[track_caller]
    fn convert(path: &str) -> String {
      url_from_directory_path_wasm(Path::new(path))
        .unwrap()
        .to_string()
    }

    assert_eq!(convert("/a/b/c"), "file:///a/b/c/");
    assert_eq!(convert("D:\\test\\other"), "file:///D:/test/other/");
  }
}
