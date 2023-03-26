use crate::flags::{Display, Flags, Layout};
use crate::meta::date::Date;
use crate::meta::filetype::FileType;
use crate::meta::name::Name;
use crate::meta::size::Size;

use std::io::{self, Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use futures::TryStreamExt;

use opendal::layers::RetryLayer;
use opendal::Entry;
use opendal::Operator;
use opendal::{services, Metadata};

pub struct DAL<'a> {
    work_dir: &'a str,

    op: Operator,
}

impl<'a> DAL<'a> {
    pub fn new(work_dir: &'a str) -> Self {
        let mut builder = services::Fs::default();
        builder.root(work_dir);
        let op: Operator = Operator::new(builder)
            .unwrap()
            .layer(RetryLayer::new())
            .finish();

        DAL { work_dir, op }
    }

    pub async fn recurse_into(
        &self,
        src: &Meta,
        depth: usize,
        flags: &Flags,
    ) -> io::Result<Vec<Meta>> {
        if depth == 0 {
            return Ok(vec![]);
        }
        if flags.display == Display::DirectoryOnly && flags.layout != Layout::Tree {
            return Ok(vec![]);
        }

        let meta = self.op.metadata(&src.entry, None).await?;
        if meta.is_file() {
            return Ok(vec![]);
        }

        let mut subs: Vec<Meta> = Vec::new();
        let mut ds = self
            .op
            .list(format!("{}/", src.entry.path()).as_str())
            .await?;
        while let Some(de) = ds.try_next().await? {
            let path = Path::new(de.path());
            let entry = self.from_path(path).await?;
            let name = path
                .file_name()
                .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "invalid file name"))?;
            if flags.ignore_globs.0.is_match(name) {
                continue;
            }
            // skip files for --tree -d
            if flags.layout == Layout::Tree
                && flags.display == Display::DirectoryOnly
                && entry.file_type() == FileType::Directory
            {
                continue;
            }

            subs.push(entry);
        }
        Ok(subs)
    }

    pub fn calculate_total_size(&mut self) {
        // if self.size.is_none() {
        //     return;
        // }
        //
        // if let FileType::Directory { .. } = self.file_type {
        //     if let Some(metas) = &mut self.content {
        //         let mut size_accumulated = match &self.size {
        //             Some(size) => size.get_bytes(),
        //             None => 0,
        //         };
        //         for x in &mut metas.iter_mut() {
        //             x.calculate_total_size();
        //             size_accumulated += match &x.size {
        //                 Some(size) => size.get_bytes(),
        //                 None => 0,
        //             };
        //         }
        //         self.size = Some(Size::new(size_accumulated));
        //     } else {
        //         // possibility that 'depth' limited the recursion in 'recurse_into'
        //         self.size = Some(Size::new(Meta::calculate_total_file_size(&self.path)));
        //     }
        // }
    }

    fn calculate_total_file_size(path: &Path) -> u64 {
        // let metadata = path.symlink_metadata();
        // let metadata = match metadata {
        //     Ok(meta) => meta,
        //     Err(err) => {
        //         print_error!("{}: {}.", path.display(), err);
        //         return 0;
        //     }
        // };
        // let file_type = metadata.file_type();
        // if file_type.is_file() {
        //     metadata.len()
        // } else if file_type.is_dir() {
        //     let mut size = metadata.len();
        //
        //     let entries = match path.read_dir() {
        //         Ok(entries) => entries,
        //         Err(err) => {
        //             print_error!("{}: {}.", path.display(), err);
        //             return size;
        //         }
        //     };
        //     for entry in entries {
        //         let path = match entry {
        //             Ok(entry) => entry.path(),
        //             Err(err) => {
        //                 print_error!("{}: {}.", path.display(), err);
        //                 continue;
        //             }
        //         };
        //         size += Meta::calculate_total_file_size(&path);
        //     }
        //     size
        // } else {
        //     0
        // }
        0
    }

    pub async fn from_path(&self, path: &Path) -> io::Result<Meta> {
        let mut builder = services::Fs::default();
        builder.root(self.work_dir);
        let m = self.op.stat(path.to_str().unwrap()).await?;
        Ok(Meta {
            entry: Entry::new(path.to_str().unwrap()),
            meta: m,
            sub_metas: vec![],
        })
    }
}

#[derive(Debug)]
pub struct Meta {
    entry: Entry,
    meta: Metadata,
    pub sub_metas: Vec<Meta>,
}

impl Meta {
    pub fn name(&self) -> Name {
        Name::new(Path::new(self.entry.path()), self.file_type())
    }

    pub fn path(&self) -> PathBuf {
        let path = Path::new(self.entry.path());
        if path.is_relative() && !path.starts_with(".") {
            // make sure start with `.` dir
            Path::new(".").join(path)
        } else {
            path.to_path_buf()
        }
    }

    pub fn size(&self) -> Option<Size> {
        Some(Size::new(self.meta.content_length()))
    }

    pub fn file_type(&self) -> FileType {
        if self.meta.is_dir() {
            return FileType::Directory;
        }

        //TODO(kw): more types
        FileType::File
    }

    pub fn modified_date(&self) -> Date {
        match self.meta.last_modified() {
            None => Date::Invalid,
            Some(offset) => Date::from(SystemTime::from(offset)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Meta;
    use std::fs::File;
    use tempfile::tempdir;

    #[cfg(unix)]
    #[test]
    fn test_from_path_path() {
        let dir = assert_fs::TempDir::new().unwrap();
        let meta = Meta::from_path(dir.path(), false).unwrap();
        assert_eq!(meta.path, dir.path())
    }

    #[test]
    fn test_from_path() {
        let tmp_dir = tempdir().expect("failed to create temp dir");

        let path_a = tmp_dir.path().join("aaa.aa");
        File::create(&path_a).expect("failed to create file");
        let meta_a = Meta::from_path(&path_a, false).expect("failed to get meta");

        let path_b = tmp_dir.path().join("bbb.bb");
        let path_c = tmp_dir.path().join("ccc.cc");

        #[cfg(unix)]
        std::os::unix::fs::symlink(path_c, &path_b).expect("failed to create broken symlink");

        // this needs to be tested on Windows
        // likely to fail because of permission issue
        // see https://doc.rust-lang.org/std/os/windows/fs/fn.symlink_file.html
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&path_c, &path_b)
            .expect("failed to create broken symlink");

        let meta_b = Meta::from_path(&path_b, true).expect("failed to get meta");

        assert!(
            meta_a.inode.is_some()
                && meta_a.links.is_some()
                && meta_a.size.is_some()
                && meta_a.date.is_some()
                && meta_a.owner.is_some()
                && meta_a.permissions.is_some()
                && meta_a.access_control.is_some()
        );

        assert!(
            meta_b.inode.is_none()
                && meta_b.links.is_none()
                && meta_b.size.is_none()
                && meta_b.date.is_none()
                && meta_b.owner.is_none()
                && meta_b.permissions.is_none()
                && meta_b.access_control.is_none()
        );
    }
}
