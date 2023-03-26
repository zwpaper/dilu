use crate::color::Colors;
use crate::dal::{Meta, DAL};
use crate::display;
use crate::flags::{ColorOption, Display, Flags, HyperlinkOption, Layout, SortOrder, ThemeOption};
use crate::icon::Icons;
use crate::{print_output, sort};
use std::path::PathBuf;

use std;
#[cfg(not(target_os = "windows"))]
use std::io;
#[cfg(not(target_os = "windows"))]
use std::os::unix::io::AsRawFd;

#[cfg(target_os = "windows")]
use terminal_size::terminal_size;

pub struct Core {
    flags: Flags,
    icons: Icons,
    colors: Colors,
    sorters: Vec<(SortOrder, sort::SortFn)>,
}

impl Core {
    pub fn new(mut flags: Flags) -> Self {
        // Check through libc if stdout is a tty. Unix specific so not on windows.
        // Determine color output availability (and initialize color output (for Windows 10))
        #[cfg(not(target_os = "windows"))]
        let tty_available = unsafe { libc::isatty(io::stdout().as_raw_fd()) == 1 };

        #[cfg(not(target_os = "windows"))]
        let console_color_ok = true;

        #[cfg(target_os = "windows")]
        let tty_available = terminal_size().is_some(); // terminal_size allows us to know if the stdout is a tty or not.

        #[cfg(target_os = "windows")]
        let console_color_ok = crossterm::ansi_support::supports_ansi();

        let mut inner_flags = flags.clone();

        let color_theme = match (tty_available && console_color_ok, flags.color.when) {
            (_, ColorOption::Never) | (false, ColorOption::Auto) => ThemeOption::NoColor,
            _ => flags.color.theme.clone(),
        };

        let icon_when = flags.icons.when;
        let icon_theme = flags.icons.theme.clone();

        // TODO: Rework this so that flags passed downstream does not
        // have Auto option for any (icon, color, hyperlink).
        if matches!(flags.hyperlink, HyperlinkOption::Auto) {
            flags.hyperlink = if tty_available {
                HyperlinkOption::Always
            } else {
                HyperlinkOption::Never
            }
        }

        let icon_separator = flags.icons.separator.0.clone();

        if !tty_available {
            // The output is not a tty, this means the command is piped. (ex: lsd -l | less)
            //
            // Most of the programs does not handle correctly the ansi colors
            // or require a raw output (like the `wc` command).
            inner_flags.layout = Layout::OneLine;

            flags.should_quote = false;
        };

        let sorters = sort::assemble_sorters(&flags);

        Self {
            flags,
            colors: Colors::new(color_theme),
            icons: Icons::new(tty_available, icon_when, icon_theme, icon_separator),
            sorters,
        }
    }

    pub async fn run(self, paths: Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
        let mut meta_list = self.fetch(paths).await?;

        self.sort(&mut meta_list);
        self.display(&meta_list);
        Ok(())
    }

    async fn fetch(&self, paths: Vec<PathBuf>) -> io::Result<Vec<Meta>> {
        let mut meta_list = Vec::with_capacity(paths.len());
        let depth = match self.flags.layout {
            Layout::Tree { .. } => self.flags.recursion.depth,
            _ if self.flags.recursion.enabled => self.flags.recursion.depth,
            _ => 1,
        };

        let pwd = std::env::current_dir().unwrap().clone();
        let work_dir = pwd.to_str().unwrap();
        for path in paths {
            let dal = DAL::new(work_dir);
            let mut meta = dal.from_path(&path).await?;

            let recurse =
                self.flags.layout == Layout::Tree || self.flags.display != Display::DirectoryOnly;
            if recurse {
                let subs = dal.recurse_into(&meta, depth, &self.flags).await?;
                meta.sub_metas = subs;
            }
            meta_list.push(meta);
        }
        // TODO(kw): calculate size
        // // Only calculate the total size of a directory if it will be displayed
        // if self.flags.total_size.0 && self.flags.blocks.displays_size() {
        //     for meta in &mut meta_list.iter_mut() {
        //         println!("size")
        //         // meta.calculate_total_size();
        //     }
        // }

        Ok(meta_list)
    }

    fn sort(&self, metas: &mut Vec<Meta>) {
        metas.sort_unstable_by(|a, b| sort::by_meta(&self.sorters, a, b));

        // for meta in metas {
        //     if let Some(ref mut content) = meta.content {
        //         self.sort(content);
        //     }
        // }
    }

    fn display(&self, metas: &[Meta]) {
        let output = if self.flags.layout == Layout::Tree {
            display::tree(metas, &self.flags, &self.colors, &self.icons)
        } else {
            display::grid(metas, &self.flags, &self.colors, &self.icons)
        };

        print_output!("{}", output);
    }
}
