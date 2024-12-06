// Copyright 2018-2024 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use boxed_error::Boxed;
use data_url::DataUrl;
use deno_error::JsError;
use deno_media_type::MediaType;
use deno_path_util::url_to_file_path;
use http::header;
use http::header::ACCEPT;
use http::header::AUTHORIZATION;
use http::header::IF_NONE_MATCH;
use http::header::LOCATION;
use http::HeaderMap;
use http::HeaderValue;
use log::debug;
use thiserror::Error;
use url::Url;

use self::http_util::CacheSemantics;
use crate::common::HeadersMap;
use crate::CacheEntry;
use crate::CacheReadFileError;
use crate::Checksum;
use crate::ChecksumIntegrityError;
use crate::DenoCacheEnv;
use crate::HttpCache;

mod auth_tokens;
mod http_util;

pub use auth_tokens::AuthDomain;
pub use auth_tokens::AuthToken;
pub use auth_tokens::AuthTokenData;
pub use auth_tokens::AuthTokens;

/// Indicates how cached source files should be handled.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CacheSetting {
  /// Only the cached files should be used.  Any files not in the cache will
  /// error.  This is the equivalent of `--cached-only` in the CLI.
  Only,
  /// No cached source files should be used, and all files should be reloaded.
  /// This is the equivalent of `--reload` in the CLI.
  ReloadAll,
  /// Only some cached resources should be used.  This is the equivalent of
  /// `--reload=jsr:@std/http/file-server` or
  /// `--reload=jsr:@std/http/file-server,jsr:@std/assert/assert-equals`.
  ReloadSome(Vec<String>),
  /// The usability of a cached value is determined by analyzing the cached
  /// headers and other metadata associated with a cached response, reloading
  /// any cached "non-fresh" cached responses.
  RespectHeaders,
  /// The cached source files should be used for local modules.  This is the
  /// default behavior of the CLI.
  Use,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum FileOrRedirect {
  File(File),
  Redirect(Url),
}

impl FileOrRedirect {
  fn from_deno_cache_entry(
    specifier: &Url,
    cache_entry: CacheEntry,
  ) -> Result<Self, RedirectResolutionError> {
    if let Some(redirect_to) = cache_entry.metadata.headers.get("location") {
      let redirect = specifier.join(redirect_to).map_err(|source| {
        RedirectResolutionError {
          specifier: specifier.clone(),
          location: redirect_to.clone(),
          source,
        }
      })?;
      Ok(FileOrRedirect::Redirect(redirect))
    } else {
      Ok(FileOrRedirect::File(File {
        specifier: specifier.clone(),
        maybe_headers: Some(cache_entry.metadata.headers),
        source: Arc::from(cache_entry.content),
      }))
    }
  }
}

/// A structure representing a source file.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct File {
  /// The _final_ specifier for the file.  The requested specifier and the final
  /// specifier maybe different for remote files that have been redirected.
  pub specifier: Url,
  pub maybe_headers: Option<HashMap<String, String>>,
  /// The source of the file.
  pub source: Arc<[u8]>,
}

impl File {
  pub fn resolve_media_type_and_charset(&self) -> (MediaType, Option<&str>) {
    deno_media_type::resolve_media_type_and_charset_from_content_type(
      &self.specifier,
      self
        .maybe_headers
        .as_ref()
        .and_then(|h| h.get("content-type"))
        .map(|v| v.as_str()),
    )
  }
}

pub trait MemoryFiles: std::fmt::Debug + Send + Sync {
  fn get(&self, specifier: &Url) -> Option<File>;
}

/// Implementation of `MemoryFiles` that always returns `None`.
#[derive(Debug, Clone, Default)]
pub struct NullMemoryFiles;

impl MemoryFiles for NullMemoryFiles {
  fn get(&self, _specifier: &Url) -> Option<File> {
    None
  }
}

#[derive(Debug)]
pub enum SendResponse {
  NotModified,
  Redirect(HeaderMap),
  Success(HeaderMap, Vec<u8>),
}

#[derive(Debug)]
pub enum SendError {
  Failed(Box<dyn std::error::Error + Send + Sync>),
  NotFound,
  StatusCode(http::StatusCode),
}

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error("Failed resolving redirect from '{specifier}' to '{location}'.")]
pub struct RedirectResolutionError {
  pub specifier: Url,
  pub location: String,
  #[source]
  pub source: url::ParseError,
}

#[derive(Debug, Error, JsError)]
#[class(uri)]
#[error("Unable to decode data url.")]
pub struct DataUrlDecodeError {
  #[source]
  source: DataUrlDecodeSourceError,
}

#[derive(Debug, Error, JsError)]
#[class(uri)]
pub enum DataUrlDecodeSourceError {
  #[error(transparent)]
  DataUrl(data_url::DataUrlError),
  #[error(transparent)]
  InvalidBase64(data_url::forgiving_base64::InvalidBase64),
}

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error("Failed reading cache entry for '{specifier}'.")]
pub struct CacheReadError {
  pub specifier: Url,
  #[source]
  pub source: std::io::Error,
}

#[derive(Debug, Error)]
#[error("Failed reading location header for '{}'", .request_url)]
pub struct FailedReadingRedirectHeaderError {
  pub request_url: Url,
  #[source]
  pub maybe_source: Option<header::ToStrError>,
}

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error("Import '{specifier}' failed.")]
pub struct FailedReadingLocalFileError {
  pub specifier: Url,
  #[source]
  pub source: std::io::Error,
}

#[derive(Debug, Error, JsError)]
#[class("Http")]
#[error("Fetch '{0}' failed, too many redirects.")]
pub struct TooManyRedirectsError(pub Url);

// this message list additional `npm` and `jsr` schemes, but they should actually be handled
// before `file_fetcher.rs` APIs are even hit.
#[derive(Debug, Error, JsError)]
#[class(type)]
#[error("Unsupported scheme \"{scheme}\" for module \"{specifier}\". Supported schemes:\n - \"blob\"\n - \"data\"\n - \"file\"\n - \"http\"\n - \"https\"\n - \"jsr\"\n - \"npm\"")]
pub struct UnsupportedSchemeError {
  pub scheme: String,
  pub specifier: Url,
}

/// Gets if the provided scheme was valid.
pub fn is_valid_scheme(scheme: &str) -> bool {
  matches!(
    scheme,
    "blob" | "data" | "file" | "http" | "https" | "jsr" | "npm"
  )
}

#[derive(Debug, Boxed, JsError)]
pub struct FetchNoFollowError(pub Box<FetchNoFollowErrorKind>);

#[derive(Debug, Error, JsError)]
pub enum FetchNoFollowErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  UrlToFilePath(#[from] deno_path_util::UrlToFilePathError),
  #[class("NotFound")]
  #[error("Import '{0}' failed, not found.")]
  NotFound(Url),
  #[class(generic)]
  #[error("Import '{specifier}' failed.")]
  ReadingBlobUrl {
    specifier: Url,
    #[source]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error(transparent)]
  ReadingFile(#[from] FailedReadingLocalFileError),
  #[class(generic)]
  #[error("Import '{specifier}' failed.")]
  FetchingRemote {
    specifier: Url,
    #[source]
    source: Box<dyn std::error::Error + Send + Sync>,
  },
  #[class(generic)]
  #[error("Import '{specifier}' failed: {status_code}")]
  ClientError {
    specifier: Url,
    status_code: http::StatusCode,
  },
  #[class("NoRemote")]
  #[error(
    "A remote specifier was requested: \"{0}\", but --no-remote is specified."
  )]
  NoRemote(Url),
  #[class(inherit)]
  #[error(transparent)]
  DataUrlDecode(DataUrlDecodeError),
  #[class(inherit)]
  #[error(transparent)]
  RedirectResolution(#[from] RedirectResolutionError),
  #[class(inherit)]
  #[error(transparent)]
  ChecksumIntegrity(#[from] ChecksumIntegrityError),
  #[class(inherit)]
  #[error(transparent)]
  CacheRead(#[from] CacheReadError),
  #[class(generic)]
  #[error("Failed caching '{specifier}'.")]
  CacheSave {
    specifier: Url,
    #[source]
    source: std::io::Error,
  },
  // this message list additional `npm` and `jsr` schemes, but they should actually be handled
  // before `file_fetcher.rs` APIs are even hit.
  #[class(inherit)]
  #[error(transparent)]
  UnsupportedScheme(#[from] UnsupportedSchemeError),
  #[class(type)]
  #[error(transparent)]
  FailedReadingRedirectHeader(#[from] FailedReadingRedirectHeaderError),
  #[class("NotCached")]
  #[error("Specifier not found in cache: \"{specifier}\", --cached-only is specified.")]
  NotCached { specifier: Url },
  #[class(type)]
  #[error("Failed setting header '{name}'.")]
  InvalidHeader {
    name: &'static str,
    #[source]
    source: header::InvalidHeaderValue,
  },
}

#[derive(Debug, Boxed, JsError)]
pub struct FetchCachedError(pub Box<FetchCachedErrorKind>);

#[derive(Debug, Error, JsError)]
pub enum FetchCachedErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  TooManyRedirects(TooManyRedirectsError),
  #[class(inherit)]
  #[error(transparent)]
  ChecksumIntegrity(#[from] ChecksumIntegrityError),
  #[class(inherit)]
  #[error(transparent)]
  CacheRead(#[from] CacheReadError),
  #[class(inherit)]
  #[error(transparent)]
  RedirectResolution(#[from] RedirectResolutionError),
}

#[derive(Debug, Boxed, JsError)]
pub struct FetchLocalError(pub Box<FetchLocalErrorKind>);

#[derive(Debug, Error, JsError)]
pub enum FetchLocalErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  UrlToFilePath(#[from] deno_path_util::UrlToFilePathError),
  #[class(inherit)]
  #[error(transparent)]
  ReadingFile(#[from] FailedReadingLocalFileError),
}

impl From<FetchLocalError> for FetchNoFollowError {
  fn from(err: FetchLocalError) -> Self {
    match err.into_kind() {
      FetchLocalErrorKind::UrlToFilePath(err) => err.into(),
      FetchLocalErrorKind::ReadingFile(err) => err.into(),
    }
  }
}

#[derive(Debug, Boxed)]
struct FetchCachedNoFollowError(pub Box<FetchCachedNoFollowErrorKind>);

#[derive(Debug, Error)]
enum FetchCachedNoFollowErrorKind {
  #[error(transparent)]
  ChecksumIntegrity(ChecksumIntegrityError),
  #[error(transparent)]
  CacheRead(#[from] CacheReadError),
  #[error(transparent)]
  RedirectResolution(#[from] RedirectResolutionError),
}

impl From<FetchCachedNoFollowError> for FetchCachedError {
  fn from(err: FetchCachedNoFollowError) -> Self {
    match err.into_kind() {
      FetchCachedNoFollowErrorKind::ChecksumIntegrity(err) => err.into(),
      FetchCachedNoFollowErrorKind::CacheRead(err) => err.into(),
      FetchCachedNoFollowErrorKind::RedirectResolution(err) => err.into(),
    }
  }
}

impl From<FetchCachedNoFollowError> for FetchNoFollowError {
  fn from(err: FetchCachedNoFollowError) -> Self {
    match err.into_kind() {
      FetchCachedNoFollowErrorKind::ChecksumIntegrity(err) => err.into(),
      FetchCachedNoFollowErrorKind::CacheRead(err) => err.into(),
      FetchCachedNoFollowErrorKind::RedirectResolution(err) => err.into(),
    }
  }
}

#[async_trait::async_trait(?Send)]
pub trait HttpClient: std::fmt::Debug + Send + Sync {
  /// Send a request getting the response.
  ///
  /// The implementation MUST not follow redirects. Return `SendResponse::Redirect`
  /// in that case.
  ///
  /// The implementation may retry the request on failure.
  async fn send_no_follow(
    &self,
    url: &Url,
    headers: HeaderMap,
  ) -> Result<SendResponse, SendError>;
}

#[derive(Debug, Clone)]
pub struct BlobData {
  pub media_type: String,
  pub bytes: Vec<u8>,
}

#[async_trait::async_trait(?Send)]
pub trait BlobStore: std::fmt::Debug + Send + Sync {
  async fn get(&self, specifier: &Url) -> std::io::Result<Option<BlobData>>;
}

#[derive(Debug, Default)]
pub struct FetchNoFollowOptions<'a> {
  pub maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
  pub maybe_checksum: Option<Checksum<'a>>,
  pub maybe_accept: Option<&'a str>,
  pub maybe_cache_setting: Option<&'a CacheSetting>,
}

#[derive(Debug)]
pub struct FileFetcherOptions {
  pub allow_remote: bool,
  pub cache_setting: CacheSetting,
  pub auth_tokens: AuthTokens,
}

/// A structure for resolving, fetching and caching source files.
#[derive(Debug)]
pub struct FileFetcher<
  TBlobStore: BlobStore,
  TEnv: DenoCacheEnv,
  THttpClient: HttpClient,
> {
  blob_store: TBlobStore,
  env: TEnv,
  http_cache: Arc<dyn HttpCache>,
  http_client: THttpClient,
  memory_files: Arc<dyn MemoryFiles>,
  allow_remote: bool,
  cache_setting: CacheSetting,
  auth_tokens: AuthTokens,
}

impl<TBlobStore: BlobStore, TEnv: DenoCacheEnv, THttpClient: HttpClient>
  FileFetcher<TBlobStore, TEnv, THttpClient>
{
  pub fn new(
    blob_store: TBlobStore,
    env: TEnv,
    http_cache: Arc<dyn HttpCache>,
    http_client: THttpClient,
    memory_files: Arc<dyn MemoryFiles>,
    options: FileFetcherOptions,
  ) -> Self {
    Self {
      blob_store,
      env,
      http_cache,
      http_client,
      memory_files,
      allow_remote: options.allow_remote,
      auth_tokens: options.auth_tokens,
      cache_setting: options.cache_setting,
    }
  }

  pub fn cache_setting(&self) -> &CacheSetting {
    &self.cache_setting
  }

  /// Fetch cached remote file.
  pub fn fetch_cached(
    &self,
    specifier: &Url,
    redirect_limit: i64,
  ) -> Result<Option<File>, FetchCachedError> {
    if !matches!(specifier.scheme(), "http" | "https") {
      return Ok(None);
    }

    let mut specifier = Cow::Borrowed(specifier);
    for _ in 0..=redirect_limit {
      match self.fetch_cached_no_follow(&specifier, None)? {
        Some(FileOrRedirect::File(file)) => {
          return Ok(Some(file));
        }
        Some(FileOrRedirect::Redirect(redirect_specifier)) => {
          specifier = Cow::Owned(redirect_specifier);
        }
        None => {
          return Ok(None);
        }
      }
    }
    Err(
      FetchCachedErrorKind::TooManyRedirects(TooManyRedirectsError(
        specifier.into_owned(),
      ))
      .into_box(),
    )
  }

  /// Fetches without following redirects.
  ///
  /// You should verify permissions of the specifier before calling this function.
  pub async fn fetch_no_follow(
    &self,
    specifier: &Url,
    options: FetchNoFollowOptions<'_>,
  ) -> Result<FileOrRedirect, FetchNoFollowError> {
    // note: this debug output is used by the tests
    debug!("FileFetcher::fetch_no_follow - specifier: {}", specifier);
    let scheme = specifier.scheme();
    if let Some(file) = self.memory_files.get(specifier) {
      Ok(FileOrRedirect::File(file))
    } else if scheme == "file" {
      // we do not in memory cache files, as this would prevent files on the
      // disk changing effecting things like workers and dynamic imports.
      let maybe_file = self.fetch_local(specifier)?;
      match maybe_file {
        Some(file) => Ok(FileOrRedirect::File(file)),
        None => {
          Err(FetchNoFollowErrorKind::NotFound(specifier.clone()).into_box())
        }
      }
    } else if scheme == "data" {
      self
        .fetch_data_url(specifier)
        .map(FileOrRedirect::File)
        .map_err(|e| FetchNoFollowErrorKind::DataUrlDecode(e).into_box())
    } else if scheme == "blob" {
      self
        .fetch_blob_url(specifier)
        .await
        .map(FileOrRedirect::File)
    } else if scheme == "https" || scheme == "http" {
      if !self.allow_remote {
        Err(FetchNoFollowErrorKind::NoRemote(specifier.clone()).into_box())
      } else {
        self
          .fetch_remote_no_follow(
            specifier,
            options.maybe_accept,
            options.maybe_cache_setting.unwrap_or(&self.cache_setting),
            options.maybe_checksum,
            options.maybe_auth,
          )
          .await
      }
    } else {
      Err(
        FetchNoFollowErrorKind::UnsupportedScheme(UnsupportedSchemeError {
          scheme: scheme.to_string(),
          specifier: specifier.clone(),
        })
        .into_box(),
      )
    }
  }

  fn fetch_cached_no_follow(
    &self,
    specifier: &Url,
    maybe_checksum: Option<Checksum<'_>>,
  ) -> Result<Option<FileOrRedirect>, FetchCachedNoFollowError> {
    debug!(
      "FileFetcher::fetch_cached_no_follow - specifier: {}",
      specifier
    );

    let cache_key =
      self
        .http_cache
        .cache_item_key(specifier)
        .map_err(|source| CacheReadError {
          specifier: specifier.clone(),
          source,
        })?;
    match self.http_cache.get(&cache_key, maybe_checksum) {
      Ok(Some(entry)) => Ok(Some(FileOrRedirect::from_deno_cache_entry(
        specifier, entry,
      )?)),
      Ok(None) => Ok(None),
      Err(CacheReadFileError::Io(source)) => Err(
        FetchCachedNoFollowErrorKind::CacheRead(CacheReadError {
          specifier: specifier.clone(),
          source,
        })
        .into_box(),
      ),
      Err(CacheReadFileError::ChecksumIntegrity(err)) => {
        Err(FetchCachedNoFollowErrorKind::ChecksumIntegrity(*err).into_box())
      }
    }
  }

  /// Convert a data URL into a file, resulting in an error if the URL is
  /// invalid.
  fn fetch_data_url(
    &self,
    specifier: &Url,
  ) -> Result<File, DataUrlDecodeError> {
    fn parse(
      specifier: &Url,
    ) -> Result<(Vec<u8>, HashMap<String, String>), DataUrlDecodeError> {
      let url = DataUrl::process(specifier.as_str()).map_err(|source| {
        DataUrlDecodeError {
          source: DataUrlDecodeSourceError::DataUrl(source),
        }
      })?;
      let (bytes, _) =
        url.decode_to_vec().map_err(|source| DataUrlDecodeError {
          source: DataUrlDecodeSourceError::InvalidBase64(source),
        })?;
      let headers = HashMap::from([(
        "content-type".to_string(),
        url.mime_type().to_string(),
      )]);
      Ok((bytes, headers))
    }

    debug!("FileFetcher::fetch_data_url() - specifier: {}", specifier);
    let (bytes, headers) = parse(specifier)?;
    Ok(File {
      specifier: specifier.clone(),
      maybe_headers: Some(headers),
      source: Arc::from(bytes),
    })
  }

  /// Get a blob URL.
  async fn fetch_blob_url(
    &self,
    specifier: &Url,
  ) -> Result<File, FetchNoFollowError> {
    debug!("FileFetcher::fetch_blob_url() - specifier: {}", specifier);
    let blob = self
      .blob_store
      .get(specifier)
      .await
      .map_err(|err| FetchNoFollowErrorKind::ReadingBlobUrl {
        specifier: specifier.clone(),
        source: err,
      })?
      .ok_or_else(|| FetchNoFollowErrorKind::NotFound(specifier.clone()))?;

    let headers =
      HashMap::from([("content-type".to_string(), blob.media_type.clone())]);

    Ok(File {
      specifier: specifier.clone(),
      maybe_headers: Some(headers),
      source: Arc::from(blob.bytes),
    })
  }

  async fn fetch_remote_no_follow(
    &self,
    specifier: &Url,
    maybe_accept: Option<&str>,
    cache_setting: &CacheSetting,
    maybe_checksum: Option<Checksum<'_>>,
    maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
  ) -> Result<FileOrRedirect, FetchNoFollowError> {
    debug!(
      "FileFetcher::fetch_remote_no_follow - specifier: {}",
      specifier
    );

    if self.should_use_cache(specifier, cache_setting) {
      if let Some(file_or_redirect) =
        self.fetch_cached_no_follow(specifier, maybe_checksum)?
      {
        return Ok(file_or_redirect);
      }
    }

    if *cache_setting == CacheSetting::Only {
      return Err(
        FetchNoFollowErrorKind::NotCached {
          specifier: specifier.clone(),
        }
        .into_box(),
      );
    }

    let maybe_etag_cache_entry = self
      .http_cache
      .cache_item_key(specifier)
      .ok()
      .and_then(|key| self.http_cache.get(&key, maybe_checksum).ok().flatten())
      .and_then(|mut cache_entry| {
        cache_entry
          .metadata
          .headers
          .remove("etag")
          .map(|etag| (cache_entry, etag))
      });

    let maybe_auth_token = self.auth_tokens.get(specifier);
    match self
      .send_request(SendRequestArgs {
        url: specifier,
        maybe_accept,
        maybe_auth: maybe_auth.clone(),
        maybe_auth_token,
        maybe_etag: maybe_etag_cache_entry
          .as_ref()
          .map(|(_, etag)| etag.as_str()),
      })
      .await?
    {
      SendRequestResponse::NotModified => {
        let (cache_entry, _) = maybe_etag_cache_entry.unwrap();
        FileOrRedirect::from_deno_cache_entry(specifier, cache_entry).map_err(
          |err| FetchNoFollowErrorKind::RedirectResolution(err).into_box(),
        )
      }
      SendRequestResponse::Redirect(redirect_url, headers) => {
        self
          .http_cache
          .set(specifier, headers, &[])
          .map_err(|source| FetchNoFollowErrorKind::CacheSave {
            specifier: specifier.clone(),
            source,
          })?;
        Ok(FileOrRedirect::Redirect(redirect_url))
      }
      SendRequestResponse::Code(bytes, headers) => {
        self
          .http_cache
          .set(specifier, headers.clone(), &bytes)
          .map_err(|source| FetchNoFollowErrorKind::CacheSave {
            specifier: specifier.clone(),
            source,
          })?;
        if let Some(checksum) = &maybe_checksum {
          checksum
            .check(specifier, &bytes)
            .map_err(|err| FetchNoFollowErrorKind::ChecksumIntegrity(*err))?;
        }
        Ok(FileOrRedirect::File(File {
          specifier: specifier.clone(),
          maybe_headers: Some(headers),
          source: Arc::from(bytes),
        }))
      }
    }
  }

  /// Returns if the cache should be used for a given specifier.
  fn should_use_cache(
    &self,
    specifier: &Url,
    cache_setting: &CacheSetting,
  ) -> bool {
    match cache_setting {
      CacheSetting::ReloadAll => false,
      CacheSetting::Use | CacheSetting::Only => true,
      CacheSetting::RespectHeaders => {
        let Ok(cache_key) = self.http_cache.cache_item_key(specifier) else {
          return false;
        };
        let Ok(Some(headers)) = self.http_cache.read_headers(&cache_key) else {
          return false;
        };
        let Ok(Some(download_time)) =
          self.http_cache.read_download_time(&cache_key)
        else {
          return false;
        };
        let cache_semantics =
          CacheSemantics::new(headers, download_time, self.env.time_now());
        cache_semantics.should_use()
      }
      CacheSetting::ReloadSome(list) => {
        let mut url = specifier.clone();
        url.set_fragment(None);
        if list.iter().any(|x| x == url.as_str()) {
          return false;
        }
        url.set_query(None);
        let mut path = PathBuf::from(url.as_str());
        loop {
          if list.contains(&path.to_str().unwrap().to_string()) {
            return false;
          }
          if !path.pop() {
            break;
          }
        }
        true
      }
    }
  }

  /// Asynchronously fetches the given HTTP URL one pass only.
  /// If no redirect is present and no error occurs,
  /// yields Code(ResultPayload).
  /// If redirect occurs, does not follow and
  /// yields Redirect(url).
  async fn send_request<'a>(
    &self,
    args: SendRequestArgs<'a>,
  ) -> Result<SendRequestResponse, FetchNoFollowError> {
    let mut headers = HeaderMap::with_capacity(3);

    if let Some(etag) = args.maybe_etag {
      let if_none_match_val =
        HeaderValue::from_str(etag).map_err(|source| {
          FetchNoFollowErrorKind::InvalidHeader {
            name: "etag",
            source,
          }
        })?;
      headers.insert(IF_NONE_MATCH, if_none_match_val);
    }
    if let Some(auth_token) = args.maybe_auth_token {
      let authorization_val = HeaderValue::from_str(&auth_token.to_string())
        .map_err(|source| FetchNoFollowErrorKind::InvalidHeader {
          name: "authorization",
          source,
        })?;
      headers.insert(AUTHORIZATION, authorization_val);
    } else if let Some((header, value)) = args.maybe_auth {
      headers.insert(header, value);
    }
    if let Some(accept) = args.maybe_accept {
      let accepts_val = HeaderValue::from_str(accept).map_err(|source| {
        FetchNoFollowErrorKind::InvalidHeader {
          name: "accept",
          source,
        }
      })?;
      headers.insert(ACCEPT, accepts_val);
    }
    match self.http_client.send_no_follow(args.url, headers).await {
      Ok(resp) => match resp {
        SendResponse::NotModified => Ok(SendRequestResponse::NotModified),
        SendResponse::Redirect(headers) => {
          let new_url = resolve_redirect_from_response(args.url, &headers)?;
          Ok(SendRequestResponse::Redirect(
            new_url,
            response_headers_to_headers_map(headers),
          ))
        }
        SendResponse::Success(headers, body) => Ok(SendRequestResponse::Code(
          body,
          response_headers_to_headers_map(headers),
        )),
      },
      Err(err) => match err {
        SendError::Failed(err) => Err(
          FetchNoFollowErrorKind::FetchingRemote {
            specifier: args.url.clone(),
            source: err,
          }
          .into_box(),
        ),
        SendError::NotFound => {
          Err(FetchNoFollowErrorKind::NotFound(args.url.clone()).into_box())
        }
        SendError::StatusCode(status_code) => Err(
          FetchNoFollowErrorKind::ClientError {
            specifier: args.url.clone(),
            status_code,
          }
          .into_box(),
        ),
      },
    }
  }

  /// Fetch a source file from the local file system.
  pub fn fetch_local(
    &self,
    specifier: &Url,
  ) -> Result<Option<File>, FetchLocalError> {
    let local = url_to_file_path(specifier)?;
    // If it doesnt have a extension, we want to treat it as typescript by default
    let headers = if local.extension().is_none() {
      Some(HashMap::from([(
        "content-type".to_string(),
        "application/typescript".to_string(),
      )]))
    } else {
      None
    };
    let bytes = match self.env.read_file_bytes(&local) {
      Ok(bytes) => bytes,
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
        return Ok(None);
      }
      Err(err) => {
        return Err(
          FetchLocalErrorKind::ReadingFile(FailedReadingLocalFileError {
            specifier: specifier.clone(),
            source: err,
          })
          .into_box(),
        );
      }
    };

    Ok(Some(File {
      specifier: specifier.clone(),
      maybe_headers: headers,
      source: bytes.into(),
    }))
  }
}

fn response_headers_to_headers_map(response_headers: HeaderMap) -> HeadersMap {
  let mut result_headers = HashMap::with_capacity(response_headers.len());
  // todo(THIS PR): change to consume to avoid allocations
  for key in response_headers.keys() {
    let key_str = key.to_string();
    let values = response_headers.get_all(key);
    // todo(dsherret): this seems very strange storing them comma separated
    // like this... what happens if a value contains a comma?
    let values_str = values
      .iter()
      .filter_map(|e| Some(e.to_str().ok()?.to_string()))
      .collect::<Vec<String>>()
      .join(",");
    result_headers.insert(key_str, values_str);
  }
  result_headers
}

fn resolve_redirect_from_response(
  request_url: &Url,
  headers: &HeaderMap,
) -> Result<Url, FailedReadingRedirectHeaderError> {
  if let Some(location) = headers.get(LOCATION) {
    let location_string =
      location
        .to_str()
        .map_err(|source| FailedReadingRedirectHeaderError {
          request_url: request_url.clone(),
          maybe_source: Some(source),
        })?;
    log::debug!("Redirecting to {:?}...", &location_string);
    let new_url = resolve_url_from_location(request_url, location_string);
    Ok(new_url)
  } else {
    Err(FailedReadingRedirectHeaderError {
      request_url: request_url.clone(),
      maybe_source: None,
    })
  }
}

/// Construct the next uri based on base uri and location header fragment
/// See <https://tools.ietf.org/html/rfc3986#section-4.2>
fn resolve_url_from_location(base_url: &Url, location: &str) -> Url {
  if location.starts_with("http://") || location.starts_with("https://") {
    // absolute uri
    Url::parse(location).expect("provided redirect url should be a valid url")
  } else if location.starts_with("//") {
    // "//" authority path-abempty
    Url::parse(&format!("{}:{}", base_url.scheme(), location))
      .expect("provided redirect url should be a valid url")
  } else if location.starts_with('/') {
    // path-absolute
    base_url
      .join(location)
      .expect("provided redirect url should be a valid url")
  } else {
    // assuming path-noscheme | path-empty
    let base_url_path_str = base_url.path().to_owned();
    // Pop last part or url (after last slash)
    let segs: Vec<&str> = base_url_path_str.rsplitn(2, '/').collect();
    let new_path = format!("{}/{}", segs.last().unwrap_or(&""), location);
    base_url
      .join(&new_path)
      .expect("provided redirect url should be a valid url")
  }
}

#[derive(Debug)]
struct SendRequestArgs<'a> {
  pub url: &'a Url,
  pub maybe_accept: Option<&'a str>,
  pub maybe_etag: Option<&'a str>,
  pub maybe_auth_token: Option<&'a AuthToken>,
  pub maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
}

#[derive(Debug, Eq, PartialEq)]
enum SendRequestResponse {
  Code(Vec<u8>, HeadersMap),
  NotModified,
  Redirect(Url, HeadersMap),
}
