// Copyright 2018-2025 the Deno authors. MIT license.

#![allow(clippy::disallowed_methods)]

use std::rc::Rc;

use deno_cache_dir::file_fetcher::AuthTokens;
use deno_cache_dir::file_fetcher::CacheSetting;
use deno_cache_dir::file_fetcher::FetchNoFollowErrorKind;
use deno_cache_dir::file_fetcher::FetchNoFollowOptions;
use deno_cache_dir::file_fetcher::FileFetcher;
use deno_cache_dir::file_fetcher::FileFetcherOptions;
use deno_cache_dir::file_fetcher::HttpClient;
use deno_cache_dir::file_fetcher::NullBlobStore;
use deno_cache_dir::file_fetcher::NullMemoryFiles;
use deno_cache_dir::file_fetcher::SendError;
use deno_cache_dir::file_fetcher::SendResponse;
use deno_cache_dir::memory::MemoryHttpCache;
use http::HeaderMap;
use sys_traits::impls::InMemorySys;
use url::Url;

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
) -> FileFetcher<NullBlobStore, InMemorySys, TClient> {
  FileFetcher::new(
    NullBlobStore,
    InMemorySys::default(),
    Rc::new(MemoryHttpCache::default()),
    client,
    Rc::new(NullMemoryFiles),
    FileFetcherOptions {
      allow_remote: true,
      cache_setting: CacheSetting::Use,
      auth_tokens: AuthTokens::new(None),
    },
  )
}
