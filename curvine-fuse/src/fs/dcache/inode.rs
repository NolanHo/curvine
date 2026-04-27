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

use crate::fs::dcache::{DirEntry, DirtyFlags, Lifecycle, OpState};
use crate::{err_fuse, FuseResult, FUSE_PATH_SEPARATOR, FUSE_ROOT_ID};
use curvine_common::state::{FileStatus, LocatedBlock, SetAttrOpts};
use orpc::common::LocalTime;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};

#[derive(Default, Deserialize, Serialize, Clone, Debug)]
pub struct Inode {
    pub ino: u64,
    pub parent: u64,
    pub name: String,

    pub status: FileStatus,
    pub locs: Option<Box<Vec<LocatedBlock>>>,

    pub lifecycle: Lifecycle,
    pub dirty: DirtyFlags,

    pub n_lookup: u64,
    pub ref_ctr: u64,
    pub last_access: u64,

    pub dir: Option<Box<DirEntry>>,

    pub mark_delete: bool,
}

impl Inode {
    pub fn new_root() -> Self {
        let root_st = FileStatus {
            is_dir: true,
            name: FUSE_PATH_SEPARATOR.to_owned(),
            path: FUSE_PATH_SEPARATOR.to_owned(),
            ..Default::default()
        };
        let dir = Some(Box::new(DirEntry::new()));
        Inode {
            ino: FUSE_ROOT_ID,
            parent: 0,
            name: FUSE_PATH_SEPARATOR.to_owned(),
            status: root_st,
            locs: None,
            lifecycle: Lifecycle::Cached,
            n_lookup: 0,
            ref_ctr: 0,
            last_access: LocalTime::mills(),
            dir,
            ..Default::default()
        }
    }

    pub fn with_status(ino: u64, parent: u64, name: &str, status: FileStatus) -> Self {
        let dir = if status.is_dir {
            Some(Box::new(DirEntry::new()))
        } else {
            None
        };
        Inode {
            ino,
            parent,
            name: name.to_owned(),
            status,
            locs: None,
            lifecycle: Lifecycle::Cached,
            n_lookup: 1,
            ref_ctr: 1,
            last_access: LocalTime::mills(),
            dir,
            ..Default::default()
        }
    }

    pub fn update_status(&mut self, status: FileStatus) {
        if status.is_dir {
            let needs_fresh_dir = self.dir.is_none() || !self.status.is_dir;
            if needs_fresh_dir {
                let _ = self.dir.replace(Box::new(DirEntry::new()));
            }
        } else {
            let _ = self.dir.take();
        }

        self.lifecycle = Lifecycle::Cached;
        self.status = status;
        self.last_access = LocalTime::mills();
    }

    pub fn invalid_cache(&mut self) {
        if matches!(self.lifecycle, Lifecycle::Cached) {
            self.lifecycle = Lifecycle::Invalid;
        }
    }

    pub fn is_root(&self) -> bool {
        self.ino == FUSE_ROOT_ID
    }

    pub fn add_lookup(&mut self, v: u64) -> u64 {
        self.n_lookup = self.n_lookup.saturating_add(v);
        self.last_access = LocalTime::mills();
        self.n_lookup
    }

    pub fn sub_lookup(&mut self, v: u64) -> u64 {
        self.n_lookup = self.n_lookup.saturating_sub(v);
        self.n_lookup
    }

    pub fn add_ref(&mut self, v: u64) -> u64 {
        self.ref_ctr = self.ref_ctr.saturating_add(v);
        self.ref_ctr
    }

    pub fn sub_ref(&mut self, v: u64) -> u64 {
        self.ref_ctr = self.ref_ctr.saturating_sub(v);
        self.ref_ctr
    }

    pub fn should_unref(&self) -> bool {
        self.n_lookup == 0 && self.ref_ctr == 0 && !self.is_root()
    }

    pub fn sub_link(&mut self, v: u32) {
        self.nlink = self.nlink.saturating_sub(v);
    }

    pub fn add_link(&mut self, v: u32) {
        self.nlink = self.nlink.saturating_add(v);
    }

    pub fn ensure_dir_empty(&self) -> FuseResult<()> {
        if !self.is_dir {
            return Ok(());
        }
        let Some(dir) = self.dir.as_ref() else {
            return err_fuse!(libc::EIO, "directory inode {} missing DirEntry", self.ino);
        };
        if !dir.children.is_empty() {
            return err_fuse!(
                libc::ENOTEMPTY,
                "directory inode {} still has cached children",
                self.ino
            );
        }
        Ok(())
    }

    pub fn ensure_file(&self) -> FuseResult<()> {
        if self.is_dir {
            err_fuse!(libc::EIO, "inode {} is not a file", self.ino)
        } else {
            Ok(())
        }
    }

    pub fn is_dirty(&self) -> bool {
        matches!(self.lifecycle, Lifecycle::Dirty)
    }

    pub fn is_data_dirty(&self) -> bool {
        self.dirty.intersects(DirtyFlags::DATA | DirtyFlags::SIZE)
    }

    pub fn is_attr_dirty(&self) -> bool {
        self.dirty.contains(DirtyFlags::ATTR)
    }

    pub fn is_staging(&self) -> bool {
        self.dirty.contains(DirtyFlags::NEW)
    }

    pub fn cache_valid(&self, ttl: u64) -> bool {
        match self.lifecycle {
            Lifecycle::Cached => self.last_access + ttl >= LocalTime::mills(),
            Lifecycle::Dirty => true,
            Lifecycle::Invalid => false,
        }
    }

    pub fn mark_create(&mut self) {
        self.lifecycle = Lifecycle::Dirty;
        self.dirty.insert(DirtyFlags::NEW);
    }

    pub fn dirty_write(&mut self, len: i64) -> FuseResult<()> {
        self.ensure_file()?;
        self.len = len;
        Ok(())
    }

    pub fn dirty_flush(&mut self) -> FuseResult<()> {
        self.ensure_file()?;
        self.mtime = LocalTime::mills() as i64;
        Ok(())
    }

    pub fn dirty_set_attr(&mut self, opts: SetAttrOpts) -> FuseResult<FileStatus> {
        if self.is_dirty() {
            return err_fuse!(libc::EIO, "inode {} is dirty", self.ino);
        }
        self.dirty.insert(DirtyFlags::ATTR);

        if let Some(mtime)  = opts.mtime {
            self.mtime = mtime;
        }

        if let Some(atime)  = opts.atime {
            self.atime = atime;
        }

        if let Some(mode) = opts.mode {
            self.mode = mode;
        }

        if let Some(owner) = opts.owner {
            self.owner = owner;
        }

        if let Some(group) = opts.group {
            self.group = group;
        }

        self.x_attr.extend(opts.add_x_attr);
        self.x_attr.retain(|k, _| !opts.remove_x_attr.contains(k));

        Ok(self.status.clone())
    }
}

impl Deref for Inode {
    type Target = FileStatus;

    fn deref(&self) -> &Self::Target {
        &self.status
    }
}

impl DerefMut for Inode {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.status
    }
}
