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
use dashmap::DashSet;
use orpc::sync::FastDashMap;
use orpc::sys::DataSlice;
use crate::fs::pcache::InodeCache;
use crate::FuseResult;

pub struct CacheManager {
    caches: FastDashMap<u64, Arc<InodeCache>>,
    dirty_inodes: DashSet<u64>
}

impl CacheManager {
    pub fn new() -> Self {
        CacheManager {
            caches: FastDashMap::default(),
            dirty_inodes: DashSet::new()
        }
    }

    pub fn get_or_create(&self, ino: u64) -> Arc<InodeCache> {
        self.caches
            .entry(ino)
            .or_insert(Arc::new(InodeCache::new(ino)))
            .clone()
    }

    pub fn get(&self, ino: u64) -> Option<Arc<InodeCache>> {
        self.caches.get(&ino).map(|x| x.clone())
    }


    pub fn write(&self, ino: u64, off: i64, data: DataSlice) -> FuseResult<usize> {
        if data.is_empty() {
            return Ok(0)
        }

        let page_cache = self.get_or_create(ino);
        page_cache.write(off, data)
    }

    pub fn read(&self, ino: u64, off: i64, len: usize) -> FuseResult<Option<DataSlice>> {
        let page_cache = self.get_or_create(ino);
        page_cache.read(off, len)
    }
}