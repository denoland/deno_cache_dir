// Copyright 2018-2024 the Deno authors. MIT license.

#![allow(clippy::disallowed_methods)]

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use deno_cache_dir::file_fetcher::AuthTokens;
use deno_cache_dir::file_fetcher::BlobData;
use deno_cache_dir::file_fetcher::BlobStore;
use deno_cache_dir::file_fetcher::CacheSetting;
use deno_cache_dir::file_fetcher::FetchNoFollowErrorKind;
use deno_cache_dir::file_fetcher::FetchNoFollowOptions;
use deno_cache_dir::file_fetcher::FileFetcher;
use deno_cache_dir::file_fetcher::FileFetcherOptions;
use deno_cache_dir::file_fetcher::HttpClient;
use deno_cache_dir::file_fetcher::NullMemoryFiles;
use deno_cache_dir::file_fetcher::SendError;
use deno_cache_dir::file_fetcher::SendResponse;
use deno_cache_dir::memory::MemoryHttpCache;
use deno_cache_dir::DenoCacheEnv;
use deno_path_util::normalize_path;
use http::HeaderMap;
use parking_lot::Mutex;
use url::Url;

#[derive(Debug)]
struct EmptyBlobStore;

#[async_trait::async_trait(?Send)]
impl BlobStore for EmptyBlobStore {
  async fn get(&self, _url: &Url) -> std::io::Result<Option<BlobData>> {
    Ok(None)
  }
}

#[derive(Debug)]
struct File {
  data: Vec<u8>,
  modified: SystemTime,
}

#[derive(Debug, Default, Clone)]
struct MemoryDenoCacheEnv {
  files: Arc<Mutex<HashMap<PathBuf, File>>>,
}

impl DenoCacheEnv for MemoryDenoCacheEnv {
  fn read_file_bytes(
    &self,
    path: &Path,
  ) -> std::io::Result<Cow<'static, [u8]>> {
    match self.files.lock().get(&normalize_path(path)) {
      Some(file) => Ok(Cow::Owned(file.data.clone())),
      None => Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "file not found",
      )),
    }
  }

  fn atomic_write_file(
    &self,
    path: &Path,
    bytes: &[u8],
  ) -> std::io::Result<()> {
    self.files.lock().insert(
      path.to_path_buf(),
      File {
        data: bytes.to_vec(),
        modified: SystemTime::now(),
      },
    );
    Ok(())
  }

  fn canonicalize_path(&self, path: &Path) -> std::io::Result<PathBuf> {
    Ok(path.to_path_buf())
  }

  fn create_dir_all(&self, _path: &Path) -> std::io::Result<()> {
    Ok(())
  }

  fn modified(&self, path: &Path) -> std::io::Result<Option<SystemTime>> {
    match self.files.lock().get(&normalize_path(path)) {
      Some(file) => Ok(Some(file.modified)),
      None => Ok(None),
    }
  }

  fn is_file(&self, path: &Path) -> bool {
    self.files.lock().contains_key(&normalize_path(path))
  }

  fn time_now(&self) -> SystemTime {
    SystemTime::now()
  }
}

#[tokio::test]
async fn test_file_fetcher_redirects() {
  #[derive(Debug)]
  struct TestHttpClient;

  #[async_trait::async_trait(?Send)]
  impl HttpClient for TestHttpClient {
    async fn send_no_follow(
      &self,
      _url: &Url,
      _headers: HeaderMap,
    ) -> Result<SendResponse, SendError> {
      Ok(SendResponse::Redirect(HeaderMap::new()))
    }
  }

  let file_fetcher = create_file_fetcher(TestHttpClient);
  let result = file_fetcher
    .fetch_no_follow(
      &Url::parse("http://localhost/bad_redirect").unwrap(),
      FetchNoFollowOptions::default(),
    )
    .await;

  match result.unwrap_err().into_kind() {
    FetchNoFollowErrorKind::RedirectHeaderParse(err) => {
      assert_eq!(err.request_url.as_str(), "http://localhost/bad_redirect");
    }
    err => unreachable!("{:?}", err),
  }
}

fn create_file_fetcher<TClient: HttpClient>(
  client: TClient,
) -> FileFetcher<EmptyBlobStore, MemoryDenoCacheEnv, TClient> {
  FileFetcher::new(
    EmptyBlobStore,
    MemoryDenoCacheEnv::default(),
    Arc::new(MemoryHttpCache::default()),
    client,
    Arc::new(NullMemoryFiles),
    FileFetcherOptions {
      allow_remote: true,
      cache_setting: CacheSetting::Use,
      auth_tokens: AuthTokens::new(None),
    },
  )
}
