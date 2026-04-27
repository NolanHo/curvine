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

use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use axum::body::Bytes;
use tokio_util::bytes::BytesMut;
use curvine_client::file::FsWriter;
use curvine_client::unified::UnifiedWriter;
use orpc::handler::FrameBuf;
use orpc::sys::DataSlice;
use crate::{err_fuse, FuseResult};
use crate::fs::FuseWriter;

pub struct CacheState {
    pub mtime: i64,
    pub size: i64,
    pub buf: BytesMut,
    pub writer: Option<Arc<FuseWriter>>,
}

impl CacheState {
    pub fn new() -> Self {
        Self {
            mtime: 0,
            size: 0,
            buf: BytesMut::new(),
            writer: None,
        }
    }
}

pub struct InodeCache {
    ino: u64,
    state: RwLock<CacheState>,
}

impl InodeCache {
    pub fn new(ino: u64) -> Self {
        Self {
            ino,
            state: RwLock::new(CacheState::new()),
        }
    }

    pub fn lock_write(&self) -> RwLockWriteGuard<'_, CacheState> {
        self.state.write().unwrap()
    }

    pub fn lock_read(&self) -> RwLockReadGuard<'_, CacheState> {
        self.state.read().unwrap()
    }


    pub fn write(&self, off: i64, data: DataSlice) -> FuseResult<usize> {
        let mut state = self.lock_write();

        if off != state.buf.len() as i64 {
            return err_fuse!(
                libc::EINVAL,
                "write back only supports sequential append: offset={}, len={}",
                off,
                state.buf.len()
            );
        }

        state.buf.extend_from_slice(data.as_slice());
        Ok(data.len())
    }

    pub fn read(&self, off: i64, len: usize) -> FuseResult<Option<DataSlice>> {
        let state = self.lock_read();
        if off < 0 {
            return err_fuse!(libc::EINVAL, "offset cannot be negative");
        }

        let end = off.saturating_add(len as i64).min(state.buf.len() as i64);
        if end <= off {
            return Ok(None);
        }

        let (start, stop) = (off as usize, end as usize);
        let data = Bytes::copy_from_slice(&state.buf[start..stop]);
        Ok(Some(DataSlice::bytes(data)))
    }
}