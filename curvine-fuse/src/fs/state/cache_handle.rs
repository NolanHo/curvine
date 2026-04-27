//  Copyright 2025 OPPO.
//
//  Licensed under the Apache License, Version 2.0 (the "License");
//  you may not use this file except in compliance with the License.
//  You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.

use std::sync::Arc;
use tokio::sync::Mutex;
use curvine_common::fs::{StateReader, StateWriter};
use curvine_common::state::{FileStatus, LockFlags, OpenFlags};
use orpc::sys::DataSlice;
use crate::fs::{FuseReader, FuseWriter};
use crate::fs::operator::{Read, Write};
use crate::fs::pcache::CacheManager;
use crate::fs::state::NodeState;
use crate::FuseResult;
use crate::raw::fuse_abi::fuse_write_out;
use crate::session::FuseResponse;

pub struct CacheHandle {
    pub ino: u64,
    pub fh: u64,
    pub cache: Arc<CacheManager>,
    pub flags: OpenFlags,
    pub status: FileStatus,
    pub reader: Mutex<Option<FuseReader>>,
    pub writer: Mutex<Option<Arc<FuseWriter>>>,
}

impl CacheHandle {
    pub fn new(ino: u64, fh: u64, cache: Arc<CacheManager>, status: FileStatus, flags: u32) -> Self {
        Self {
            ino,
            fh,
            cache,
            flags: OpenFlags::new(flags),
            status,
            reader: Mutex::new(None),
            writer: Mutex::new(None),
        }
    }

    pub async fn read(
        &self,
        state: &NodeState,
        op: Read<'_>,
        reply: FuseResponse,
    ) -> FuseResult<()> {
        let data = self.cache.read(
            self.ino,
            op.arg.offset as i64,
            op.arg.size as usize
        )?;
        if let Some(data) = data {
            reply.send_data(Ok(vec![data])).await?;
            return Ok(())
        }

        let mut lock = self.reader.lock().await;
        let reader = match lock {
            None => {
                let path = state.get_path(self.ino)?;
                let reader = state.new_reader(&path).await?;
                lock.insert(reader)
            }
            Some(reader) => reader,
        };

        reader.read(op, reply).await?;
        Ok(())
    }

    pub async fn write(
        &self,
        _state: &NodeState,
        op: Write<'_>,
        reply: FuseResponse,
    ) -> FuseResult<()> {
        if op.data.is_empty() {
            return Ok(());
        }
        self.cache.write(
            self.ino,
            op.arg.offset as i64,
            DataSlice::bytes(op.data),
        )?;
        let out = fuse_write_out {
            size: op.arg.size,
            padding: 0
        };
        reply.send_rep(Ok(out)).await?;
        Ok(()
        )
    }

    pub async fn flush(&self, reply: Option<FuseResponse>) -> FuseResult<()> {
        self.complete(reply).await
    }

    pub async fn complete(&self, reply: Option<FuseResponse>) -> FuseResult<()> {
        if let Some(reply) = reply {
            reply.send_rep(Ok(())).await?;
        }
        Ok(())
    }

    pub fn status(&self) -> &FileStatus {
        &self.status
    }

    pub fn add_lock(&self, lock_flags: LockFlags, owner_id: u64)  {
        todo!()
    }

    pub fn remove_lock(&self, typ: LockFlags) -> Option<u64> {
        todo!()
    }

    pub async fn persist(&self, writer: &mut StateWriter) -> FuseResult<()> {
        todo!()
    }

    pub async fn restore(reader: &mut StateReader, state: &NodeState) -> FuseResult<Self> {
        todo!()
    }
}
