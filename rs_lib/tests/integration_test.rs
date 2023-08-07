use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use deno_cache_dir::DenoCacheFs;
use deno_cache_dir::GlobalHttpCache;
use deno_cache_dir::HttpCache;
use deno_cache_dir::LocalHttpCache;
use deno_cache_dir::LocalLspHttpCache;
use serde_json::json;
use tempfile::TempDir;
use url::Url;

#[derive(Debug, Clone)]
struct TestRealFs;

impl DenoCacheFs for TestRealFs {
  fn read_file_bytes(&self, path: &Path) -> std::io::Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
      Ok(s) => Ok(Some(s)),
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
      Err(err) => Err(err),
    }
  }

  fn atomic_write_file(&self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    match std::fs::write(path, bytes) {
      Ok(()) => Ok(()),
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(path, bytes)
      },
      Err(err) => Err(err),
    }
  }
}

#[test]
fn test_global_create_cache() {
  let dir = TempDir::new().unwrap();
  let cache_path = dir.path().join("foobar");
  // HttpCache should be created lazily on first use:
  // when zipping up a local project with no external dependencies
  // "$DENO_DIR/deps" is empty. When unzipping such project
  // "$DENO_DIR/deps" might not get restored and in situation
  // when directory is owned by root we might not be able
  // to create that directory. However if it's not needed it
  // doesn't make sense to return error in such specific scenarios.
  // For more details check issue:
  // https://github.com/denoland/deno/issues/5688
  let fs = TestRealFs;
  let cache = GlobalHttpCache::new(cache_path.clone(), fs);
  assert!(!cache.get_global_cache_location().exists());
  let url = Url::parse("http://example.com/foo/bar.js").unwrap();
  cache
    .set(&url, Default::default(), b"hello world")
    .unwrap();
  assert!(cache_path.is_dir());
  assert!(cache.get_global_cache_filepath(&url).unwrap().is_file());
}

#[test]
fn test_global_get_set() {
  let dir = TempDir::new().unwrap();
  let fs = TestRealFs;
  let cache = GlobalHttpCache::new(dir.path().to_path_buf(), fs);
  let url = Url::parse("https://deno.land/x/welcome.ts").unwrap();
  let mut headers = HashMap::new();
  headers.insert(
    "content-type".to_string(),
    "application/javascript".to_string(),
  );
  headers.insert("etag".to_string(), "as5625rqdsfb".to_string());
  let content = b"Hello world";
  cache.set(&url, headers, content).unwrap();
  let key = cache.cache_item_key(&url).unwrap();
  let content =
    String::from_utf8(cache.read_file_bytes(&key).unwrap().unwrap()).unwrap();
  let headers = cache.read_metadata(&key).unwrap().unwrap().headers;
  assert_eq!(content, "Hello world");
  assert_eq!(
    headers.get("content-type").unwrap(),
    "application/javascript"
  );
  assert_eq!(headers.get("etag").unwrap(), "as5625rqdsfb");
  assert_eq!(headers.get("foobar"), None);
}


#[test]
fn test_local_global_cache() {
  let temp_dir = TempDir::new().unwrap();
  let global_cache_path = temp_dir.path().join("global");
  let local_cache_path = temp_dir.path().join("local");
  let fs = TestRealFs;
  let global_cache =
    Arc::new(GlobalHttpCache::new(global_cache_path.clone(), fs));
  let local_cache =
    LocalHttpCache::new(local_cache_path.clone(), global_cache.clone());

  let manifest_file_path = local_cache_path.join("manifest.json");
  // mapped url
  {
    let url = Url::parse("https://deno.land/x/mod.ts").unwrap();
    let content = "export const test = 5;";
    global_cache
      .set(
        &url,
        HashMap::from([(
          "content-type".to_string(),
          "application/typescript".to_string(),
        )]),
        content.as_bytes(),
      )
      .unwrap();
    let key = local_cache.cache_item_key(&url).unwrap();
    assert_eq!(
      String::from_utf8(local_cache.read_file_bytes(&key).unwrap().unwrap())
        .unwrap(),
      content
    );
    let metadata = local_cache.read_metadata(&key).unwrap().unwrap();
    // won't have any headers because the content-type is derivable from the url
    assert_eq!(metadata.headers, HashMap::new());
    assert_eq!(metadata.url, url.to_string());
    // no manifest file yet
    assert!(!manifest_file_path.exists());

    // now try deleting the global cache and we should still be able to load it
    std::fs::remove_dir_all(&global_cache_path).unwrap();
    assert_eq!(
      String::from_utf8(local_cache.read_file_bytes(&key).unwrap().unwrap())
        .unwrap(),
      content
    );
  }

  // file that's directly mappable to a url
  {
    let content = "export const a = 1;";
    std::fs::write(local_cache_path
      .join("deno.land")
      .join("main.js"),
      content).unwrap();

    // now we should be able to read this file because it's directly mappable to a url
    let url = Url::parse("https://deno.land/main.js").unwrap();
    let key = local_cache.cache_item_key(&url).unwrap();
    assert_eq!(
      String::from_utf8(local_cache.read_file_bytes(&key).unwrap().unwrap())
        .unwrap(),
      content
    );
    let metadata = local_cache.read_metadata(&key).unwrap().unwrap();
    assert_eq!(metadata.headers, HashMap::new());
    assert_eq!(metadata.url, url.to_string());
  }

  // now try a file with a different content-type header
  {
    let url =
      Url::parse("https://deno.land/x/different_content_type.ts").unwrap();
    let content = "export const test = 5;";
    global_cache
      .set(
        &url,
        HashMap::from([(
          "content-type".to_string(),
          "application/javascript".to_string(),
        )]),
        content.as_bytes(),
      )
      .unwrap();
    let key = local_cache.cache_item_key(&url).unwrap();
    assert_eq!(
      String::from_utf8(local_cache.read_file_bytes(&key).unwrap().unwrap())
        .unwrap(),
      content
    );
    let metadata = local_cache.read_metadata(&key).unwrap().unwrap();
    assert_eq!(
      metadata.headers,
      HashMap::from([(
        "content-type".to_string(),
        "application/javascript".to_string(),
      )])
    );
    assert_eq!(metadata.url, url.to_string());
    assert_eq!(
      read_manifest(&manifest_file_path),
      json!({
        "modules": {
          "https://deno.land/x/different_content_type.ts": {
            "headers": {
              "content-type": "application/javascript"
            }
          }
        }
      })
    );
    // delete the manifest file
    std::fs::remove_file(&manifest_file_path).unwrap();

    // Now try resolving the key again and the content type should still be application/javascript.
    // This is maintained because we hash the filename when the headers don't match the extension.
    let metadata = local_cache.read_metadata(&key).unwrap().unwrap();
    assert_eq!(
      metadata.headers,
      HashMap::from([(
        "content-type".to_string(),
        "application/javascript".to_string(),
      )])
    );
  }

  // reset the local cache
  std::fs::remove_dir_all(&local_cache_path).unwrap();
  let local_cache =
    LocalHttpCache::new(local_cache_path.clone(), global_cache.clone());

  // now try caching a file with many headers
  {
    let url = Url::parse("https://deno.land/x/my_file.ts").unwrap();
    let content = "export const test = 5;";
    global_cache
      .set(
        &url,
        HashMap::from([
          (
            "content-type".to_string(),
            "application/typescript".to_string(),
          ),
          ("x-typescript-types".to_string(), "./types.d.ts".to_string()),
          ("x-deno-warning".to_string(), "Stop right now.".to_string()),
          (
            "x-other-header".to_string(),
            "Thank you very much.".to_string(),
          ),
        ]),
        content.as_bytes(),
      )
      .unwrap();
    let check_output = |local_cache: &LocalHttpCache<_>| {
      let key = local_cache.cache_item_key(&url).unwrap();
      assert_eq!(
        String::from_utf8(
          local_cache.read_file_bytes(&key).unwrap().unwrap()
        )
        .unwrap(),
        content
      );
      let metadata = local_cache.read_metadata(&key).unwrap().unwrap();
      assert_eq!(
        metadata.headers,
        HashMap::from([
          ("x-typescript-types".to_string(), "./types.d.ts".to_string(),),
          ("x-deno-warning".to_string(), "Stop right now.".to_string(),)
        ])
      );
      assert_eq!(metadata.url, url.to_string());
      assert_eq!(
        read_manifest(&manifest_file_path),
        json!({
          "modules": {
            "https://deno.land/x/my_file.ts": {
              "headers": {
                "x-deno-warning": "Stop right now.",
                "x-typescript-types": "./types.d.ts"
              }
            }
          }
        })
      );
    };
    check_output(&local_cache);
    // now ensure it's the same when re-creating the cache
    check_output(&LocalHttpCache::new(
      local_cache_path.to_path_buf(),
      global_cache.clone(),
    ));
  }

  // reset the local cache
  std::fs::remove_dir_all(&local_cache_path).unwrap();
  let local_cache =
    LocalHttpCache::new(local_cache_path.clone(), global_cache.clone());

  // try a file that can't be mapped to the file system
  {
    {
      let url =
        Url::parse("https://deno.land/INVALID/Module.ts?dev").unwrap();
      let content = "export const test = 5;";
      global_cache
        .set(&url, HashMap::new(), content.as_bytes())
        .unwrap();
      let key = local_cache.cache_item_key(&url).unwrap();
      assert_eq!(
        String::from_utf8(
          local_cache.read_file_bytes(&key).unwrap().unwrap()
        )
        .unwrap(),
        content
      );
      let metadata = local_cache.read_metadata(&key).unwrap().unwrap();
      // won't have any headers because the content-type is derivable from the url
      assert_eq!(metadata.headers, HashMap::new());
      assert_eq!(metadata.url, url.to_string());
    }

    // now try a file in the same directory, but that maps to the local filesystem
    {
      let url = Url::parse("https://deno.land/INVALID/module2.ts").unwrap();
      let content = "export const test = 4;";
      global_cache
        .set(&url, HashMap::new(), content.as_bytes())
        .unwrap();
      let key = local_cache.cache_item_key(&url).unwrap();
      assert_eq!(
        String::from_utf8(
          local_cache.read_file_bytes(&key).unwrap().unwrap()
        )
        .unwrap(),
        content
      );
      assert!(local_cache_path
        .join("deno.land/#invalid_1ee01/module2.ts")
        .exists());

      // ensure we can still read this file with a new local cache
      let local_cache = LocalHttpCache::new(
        local_cache_path.to_path_buf(),
        global_cache.clone(),
      );
      assert_eq!(
        String::from_utf8(
          local_cache.read_file_bytes(&key).unwrap().unwrap()
        )
        .unwrap(),
        content
      );
    }

    assert_eq!(
      read_manifest(&manifest_file_path),
      json!({
        "modules": {
          "https://deno.land/INVALID/Module.ts?dev": {
          }
        },
        "folders": {
          "https://deno.land/INVALID/": "deno.land/#invalid_1ee01",
        }
      })
    );
  }

  // reset the local cache
  std::fs::remove_dir_all(&local_cache_path).unwrap();
  let local_cache =
    LocalHttpCache::new(local_cache_path.clone(), global_cache.clone());

  // now try a redirect
  {
    let url = Url::parse("https://deno.land/redirect.ts").unwrap();
    global_cache
      .set(
        &url,
        HashMap::from([("location".to_string(), "./x/mod.ts".to_string())]),
        "Redirecting to other url...".as_bytes(),
      )
      .unwrap();
    let key = local_cache.cache_item_key(&url).unwrap();
    let metadata = local_cache.read_metadata(&key).unwrap().unwrap();
    assert_eq!(
      metadata.headers,
      HashMap::from([("location".to_string(), "./x/mod.ts".to_string())])
    );
    assert_eq!(metadata.url, url.to_string());
    assert_eq!(
      read_manifest(&manifest_file_path),
      json!({
        "modules": {
          "https://deno.land/redirect.ts": {
            "headers": {
              "location": "./x/mod.ts"
            }
          }
        }
      })
    );
  }
}

fn read_manifest(path: &Path) -> serde_json::Value {
  let manifest = std::fs::read_to_string(path).unwrap();
  serde_json::from_str(&manifest).unwrap()
}

#[test]
fn test_lsp_local_cache() {
  let temp_dir = TempDir::new().unwrap();
  let global_cache_path = temp_dir.path().join("global");
  let local_cache_path = temp_dir.path().join("local");
  let fs = TestRealFs;
  let global_cache =
    Arc::new(GlobalHttpCache::new(global_cache_path.to_path_buf(), fs));
  let local_cache = LocalLspHttpCache::new(
    local_cache_path.to_path_buf(),
    global_cache.clone(),
  );

  // mapped url
  {
    let url = Url::parse("https://deno.land/x/mod.ts").unwrap();
    let content = "export const test = 5;";
    global_cache
      .set(
        &url,
        HashMap::from([(
          "content-type".to_string(),
          "application/typescript".to_string(),
        )]),
        content.as_bytes(),
      )
      .unwrap();
    let key = local_cache.cache_item_key(&url).unwrap();
    assert_eq!(
      String::from_utf8(local_cache.read_file_bytes(&key).unwrap().unwrap())
        .unwrap(),
      content
    );

    // check getting the file url works
    let file_url = local_cache.get_file_url(&url);
    let expected = Url::from_directory_path(&local_cache_path).unwrap()
      .join("deno.land/x/mod.ts")
      .unwrap();
    assert_eq!(file_url, Some(expected));

    // get the reverse mapping
    let mapping = local_cache.get_remote_url(
      local_cache_path
        .join("deno.land")
        .join("x")
        .join("mod.ts")
        .as_path(),
    );
    assert_eq!(mapping.as_ref(), Some(&url));
  }

  // now try a file with a different content-type header
  {
    let url =
      Url::parse("https://deno.land/x/different_content_type.ts").unwrap();
    let content = "export const test = 5;";
    global_cache
      .set(
        &url,
        HashMap::from([(
          "content-type".to_string(),
          "application/javascript".to_string(),
        )]),
        content.as_bytes(),
      )
      .unwrap();
    let key = local_cache.cache_item_key(&url).unwrap();
    assert_eq!(
      String::from_utf8(local_cache.read_file_bytes(&key).unwrap().unwrap())
        .unwrap(),
      content
    );

    let file_url = local_cache.get_file_url(&url).unwrap();
    let path = file_url.to_file_path().unwrap();
    assert!(path.exists());
    let mapping = local_cache.get_remote_url(&path);
    assert_eq!(mapping.as_ref(), Some(&url));
  }

  // try http specifiers that can't be mapped to the file system
  {
    let urls = [
      "http://deno.land/INVALID/Module.ts?dev",
      "http://deno.land/INVALID/SubDir/Module.ts?dev",
    ];
    for url in urls {
      let url = Url::parse(url).unwrap();
      let content = "export const test = 5;";
      global_cache
        .set(&url, HashMap::new(), content.as_bytes())
        .unwrap();
      let key = local_cache.cache_item_key(&url).unwrap();
      assert_eq!(
        String::from_utf8(
          local_cache.read_file_bytes(&key).unwrap().unwrap()
        )
        .unwrap(),
        content
      );

      let file_url = local_cache.get_file_url(&url).unwrap();
      let path = file_url.to_file_path().unwrap();
      assert!(path.exists());
      let mapping = local_cache.get_remote_url(&path);
      assert_eq!(mapping.as_ref(), Some(&url));
    }

    // now try a files in the same and sub directories, that maps to the local filesystem
    let urls = [
      "http://deno.land/INVALID/module2.ts",
      "http://deno.land/INVALID/SubDir/module3.ts",
      "http://deno.land/INVALID/SubDir/sub_dir/module4.ts",
    ];
    for url in urls {
      let url = Url::parse(url).unwrap();
      let content = "export const test = 4;";
      global_cache
        .set(&url, HashMap::new(), content.as_bytes())
        .unwrap();
      let key = local_cache.cache_item_key(&url).unwrap();
      assert_eq!(
        String::from_utf8(
          local_cache.read_file_bytes(&key).unwrap().unwrap()
        )
        .unwrap(),
        content
      );
      let file_url = local_cache.get_file_url(&url).unwrap();
      let path = file_url.to_file_path().unwrap();
      assert!(path.exists());
      let mapping = local_cache.get_remote_url(&path);
      assert_eq!(mapping.as_ref(), Some(&url));

      // ensure we can still get this file with a new local cache
      let local_cache = LocalLspHttpCache::new(
        local_cache_path.to_path_buf(),
        global_cache.clone(),
      );
      let file_url = local_cache.get_file_url(&url).unwrap();
      let path = file_url.to_file_path().unwrap();
      assert!(path.exists());
      let mapping = local_cache.get_remote_url(&path);
      assert_eq!(mapping.as_ref(), Some(&url));
    }
  }
}