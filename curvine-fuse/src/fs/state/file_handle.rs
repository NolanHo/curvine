// Copyright 2025 OPPO.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::sync::Arc;
use curvine_common::fs::{StateReader, StateWriter};
use curvine_common::state::{FileStatus, LockFlags};
use orpc::sys::RawPtr;
use crate::fs::{FuseReader, FuseWriter};
use crate::fs::operator::{Read, Write};
use crate::fs::pcache::CacheManager;
use crate::fs::state::{BackendHandle, CacheHandle, NodeState};
use crate::fs::state::FileHandle::{Backend, Cache};
use crate::FuseResult;
use crate::session::FuseResponse;

pub enum FileHandle {
    Cache(CacheHandle),
    Backend(BackendHandle)
}

impl FileHandle {
    pub fn new_cache(ino: u64, fh: u64, cache: Arc<CacheManager>, status: FileStatus, flags: u32) -> Self {
        let handle = CacheHandle::new(ino, fh, cache, status, flags);
        Cache(handle)
    }

    pub fn new_backend(
        ino: u64,
        fh: u64,
        reader: Option<RawPtr<FuseReader>>,
        writer: Option<Arc<FuseWriter>>,
        status: FileStatus,
    ) -> Self {
        let handle = BackendHandle::new(ino, fh, reader, writer, status);
        Backend(handle)
    }

    pub fn has_writer(&self) -> bool {
        match self {
            Cache(h) => h.flags.write(),
            Backend(h) => h.writer.is_some()
        }
    }

    pub fn ino(&self) -> u64 {
        match self {
            Cache(h) => h.ino,
            Backend(h) => h.ino
        }
    }

    pub fn fh(&self) -> u64 {
        match self {
            Cache(h) => h.fh,
            Backend(h) => h.fh
        }
    }

    pub async fn read(
        &self,
        state: &NodeState,
        op: Read<'_>,
        reply: FuseResponse,
    ) -> FuseResult<()> {
        match self {
            Cache(h) => h.read(state, op, reply).await,
            Backend(h) => h.read(state, op, reply).await
        }
    }

    pub async fn write(&self, op: Write<'_>, state: &NodeState, reply: FuseResponse) -> FuseResult<()> {
        match self {
            Cache(h) => h.write(state, op, reply).await,
            Backend(h) => h.write(op, reply).await
        }
    }

    pub async fn flush(&self, reply: Option<FuseResponse>) -> FuseResult<()> {
        match self {
            Cache(h) => h.flush(reply).await,
            Backend(h) => h.flush(reply).await
        }
    }

    pub async fn complete(&self, reply: Option<FuseResponse>) -> FuseResult<()> {
        match self {
            Cache(h) => h.complete(reply).await,
            Backend(h) => h.complete(reply).await
        }
    }

    pub fn status(&self) -> &FileStatus {
        match self {
            Cache(h) => h.status(),
            Backend(h) => h.status()
        }
    }

    pub fn add_lock(&self, lock_flags: LockFlags, owner_id: u64) {
        match self {
            Cache(h) => h.add_lock(lock_flags, owner_id),
            Backend(h) => h.add_lock(lock_flags, owner_id)
        }
    }

    pub fn remove_lock(&self, typ: LockFlags) -> Option<u64> {
        match self {
            Cache(h) => h.remove_lock(typ),
            Backend(h) => h.remove_lock(typ)
        }
    }

    pub async fn persist(&self, writer: &mut StateWriter) -> FuseResult<()> {
        match self {
            Cache(h) =>h.persist(writer).await,
            Backend(h) => h.persist(writer).await
        }
    }

    pub async fn restore(reader: &mut StateReader, state: &NodeState) -> FuseResult<Self> {
        todo!()
    }
}
