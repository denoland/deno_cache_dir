// Copyright 2018-2024 the Deno authors. MIT license.

use sys_traits::FsCanonicalize;
use sys_traits::FsCreateDirAll;
use sys_traits::FsMetadata;
use sys_traits::FsOpen;
use sys_traits::FsRead;
use sys_traits::FsRemoveFile;
use sys_traits::FsRename;
use sys_traits::SystemRandom;
use sys_traits::SystemTimeNow;
use sys_traits::ThreadSleep;

pub trait DenoCacheSys:
  Send
  + Sync
  + std::fmt::Debug
  + Clone
  + FsCreateDirAll
  + FsCanonicalize
  + FsMetadata
  + FsOpen
  + FsRead
  + FsRemoveFile
  + FsRename
  + SystemRandom
  + SystemTimeNow
  + ThreadSleep
{
}

impl DenoCacheSys for sys_traits::impls::RealSys {}
