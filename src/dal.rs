use crate::flags::{Display, Flags, Layout};
use crate::meta::date::Date;
use crate::meta::filetype::FileType;
use crate::meta::name::Name;
use crate::meta::size::Size;
use crate::{print_error, ExitCode};

use std::io::{self, Error, ErrorKind};
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use futures::TryStreamExt;

use opendal::layers::RetryLayer;
use opendal::Entry;
use opendal::EntryMode;
use opendal::Metakey;
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

        let mut ds = self.op.list(src.entry.path()).await?;
        while let Some(mut de) = ds.try_next().await? {
            let meta = self.op.metadata(&de, Metakey::Mode).await?;
            match meta.mode() {
                EntryMode::FILE => {
                    println!("Handling file")
                }
                EntryMode::DIR => {
                    println!("Handling dir like start a new list via meta.path()")
                }
                EntryMode::Unknown => continue,
            }
        }

        // let entries = match src.path.read_dir() {
        //     Ok(entries) => entries,
        //     Err(err) => {
        //         print_error!("{}: {}.", self.path.display(), err);
        //         return Ok((None, ExitCode::MinorIssue));
        //     }
        // };
        //
        // let mut content: Vec<Meta> = Vec::new();
        //
        // if matches!(flags.display, Display::All | Display::SystemProtected)
        //     && flags.layout != Layout::Tree
        // {
        //     let mut current_meta = self.clone();
        //     current_meta.name.name = ".".to_owned();
        //
        //     let mut parent_meta =
        //         Self::from_path(&self.path.join(Component::ParentDir), flags.dereference.0)?;
        //     parent_meta.name.name = "..".to_owned();
        //
        //     content.push(current_meta);
        //     content.push(parent_meta);
        // }
        //
        // let mut exit_code = ExitCode::OK;
        //
        // for entry in entries {
        //     let entry = entry?;
        //     let path = entry.path();
        //
        //     let name = path
        //         .file_name()
        //         .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "invalid file name"))?;
        //
        //     if flags.ignore_globs.0.is_match(name) {
        //         continue;
        //     }
        //
        //     #[cfg(windows)]
        //     let is_hidden =
        //         name.to_string_lossy().starts_with('.') || windows_utils::is_path_hidden(&path);
        //     #[cfg(not(windows))]
        //     let is_hidden = name.to_string_lossy().starts_with('.');
        //
        //     #[cfg(windows)]
        //     let is_system = windows_utils::is_path_system(&path);
        //     #[cfg(not(windows))]
        //     let is_system = false;
        //
        //     match flags.display {
        //         // show hidden files, but ignore system protected files
        //         Display::All | Display::AlmostAll if is_system => continue,
        //         // ignore hidden and system protected files
        //         Display::VisibleOnly if is_hidden || is_system => continue,
        //         _ => {}
        //     }
        //
        //     let mut entry_meta = match Self::from_path(&path, flags.dereference.0) {
        //         Ok(res) => res,
        //         Err(err) => {
        //             print_error!("{}: {}.", path.display(), err);
        //             exit_code.set_if_greater(ExitCode::MinorIssue);
        //             continue;
        //         }
        //     };
        //
        //     // skip files for --tree -d
        //     if flags.layout == Layout::Tree
        //         && flags.display == Display::DirectoryOnly
        //         && !entry.file_type()?.is_dir()
        //     {
        //         continue;
        //     }
        //
        //     // check dereferencing
        //     if flags.dereference.0 || !matches!(entry_meta.file_type, FileType::SymLink { .. }) {
        //         match entry_meta.recurse_into(depth - 1, flags) {
        //             Ok((content, rec_exit_code)) => {
        //                 entry_meta.content = content;
        //                 exit_code.set_if_greater(rec_exit_code);
        //             }
        //             Err(err) => {
        //                 print_error!("{}: {}.", path.display(), err);
        //                 exit_code.set_if_greater(ExitCode::MinorIssue);
        //                 continue;
        //             }
        //         };
        //     }
        //
        //     content.push(entry_meta);
        // }
        //
        // Ok((Some(content), exit_code))
        Ok(vec![])
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
        })
    }
}

pub struct Meta {
    entry: Entry,
    meta: Metadata,
}

impl Meta {
    pub fn name(&self) -> Name {
        Name::new(Path::new(self.entry.path()), self.file_type())
    }

    pub fn path(&self) -> &Path {
        Path::new(self.entry.path())
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
