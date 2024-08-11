// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::time::SystemTime;

pub trait DenoCacheEnvFsFile {
  fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize>;
  fn seek_relative(&mut self, amount: i64) -> std::io::Result<()>;
}

pub trait DenoCacheEnv: Send + Sync + std::fmt::Debug + Clone {
  fn open_read(
    &self,
    path: &Path,
  ) -> std::io::Result<Box<dyn DenoCacheEnvFsFile>>;
  fn read_file_bytes(&self, path: &Path) -> std::io::Result<Vec<u8>>;
  fn atomic_write_file(&self, path: &Path, bytes: &[u8])
    -> std::io::Result<()>;
  fn modified(&self, path: &Path) -> std::io::Result<Option<SystemTime>>;
  fn is_file(&self, path: &Path) -> bool;
  fn time_now(&self) -> SystemTime;
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub use test_fs::TestRealDenoCacheEnv;

// allow using for this real file system
#[allow(clippy::disallowed_methods)]
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod test_fs {
  use super::*;

  struct TestRealFsFile(std::fs::File);

  impl DenoCacheEnvFsFile for TestRealFsFile {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
      use std::io::Read;
      self.0.read(bytes)
    }

    fn seek_relative(&mut self, amount: i64) -> std::io::Result<()> {
      use std::io::Seek;
      self.0.seek_relative(amount)
    }
  }

  /// An implementation of `DenoCacheEnv` that uses the real file system, but
  /// doesn't have an implementation of atomic_write_file that is resilient.
  /// This SHOULD NOT be used for production code. It's good enough for use
  /// in tests though.
  #[derive(Debug, Clone)]
  pub struct TestRealDenoCacheEnv;

  impl DenoCacheEnv for TestRealDenoCacheEnv {
    fn open_read(
      &self,
      path: &Path,
    ) -> std::io::Result<Box<dyn DenoCacheEnvFsFile>> {
      let fs_file = std::fs::File::open(path)?;
      Ok(Box::new(TestRealFsFile(fs_file)))
    }

    fn read_file_bytes(&self, path: &Path) -> std::io::Result<Vec<u8>> {
      std::fs::read(path)
    }

    fn atomic_write_file(
      &self,
      path: &Path,
      bytes: &[u8],
    ) -> std::io::Result<()> {
      match std::fs::write(path, bytes) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
          std::fs::create_dir_all(path.parent().unwrap())?;
          std::fs::write(path, bytes)
        }
        Err(err) => Err(err),
      }
    }

    fn modified(&self, path: &Path) -> std::io::Result<Option<SystemTime>> {
      match std::fs::metadata(path) {
        Ok(metadata) => Ok(Some(
          metadata.modified().unwrap_or_else(|_| SystemTime::now()),
        )),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
      }
    }

    fn is_file(&self, path: &Path) -> bool {
      path.is_file()
    }

    fn time_now(&self) -> SystemTime {
      SystemTime::now()
    }
  }
}
