// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    iced::Point,
    widget::{icon, Icon},
};
use cosmic_text::{Attrs, Buffer, Edit, Shaping, SyntaxEditor, ViEditor, Wrap};
use notify::Watcher;
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::{fl, git::GitDiff, mime_icon, Config, FALLBACK_MIME_ICON, FONT_SYSTEM, SYNTAX_SYSTEM};

pub enum Tab {
    Editor(EditorTab),
    GitDiff(GitDiffTab),
}

impl Tab {
    pub fn title(&self) -> String {
        match self {
            Self::Editor(tab) => tab.title(),
            Self::GitDiff(tab) => tab.title.clone(),
        }
    }
}

pub struct GitDiffTab {
    pub title: String,
    pub diff: GitDiff,
}

pub struct EditorTab {
    pub path_opt: Option<PathBuf>,
    attrs: Attrs<'static>,
    pub editor: Mutex<ViEditor<'static, 'static>>,
    pub context_menu: Option<Point>,
}

impl EditorTab {
    pub fn new(config: &Config) -> Self {
        //TODO: do not repeat, used in App::init
        let attrs = cosmic_text::Attrs::new().family(cosmic_text::Family::Monospace);

        let mut buffer = Buffer::new_empty(config.metrics());
        buffer.set_text(
            &mut FONT_SYSTEM.lock().unwrap(),
            "",
            attrs,
            Shaping::Advanced,
        );

        let editor =
            SyntaxEditor::new(Arc::new(buffer), &SYNTAX_SYSTEM, config.syntax_theme()).unwrap();

        let mut tab = Self {
            path_opt: None,
            attrs,
            editor: Mutex::new(ViEditor::new(editor)),
            context_menu: None,
        };

        // Update any other config settings
        tab.set_config(config);

        tab
    }

    pub fn set_config(&mut self, config: &Config) {
        let mut editor = self.editor.lock().unwrap();
        let mut font_system = FONT_SYSTEM.lock().unwrap();
        let mut editor = editor.borrow_with(&mut font_system);
        editor.set_auto_indent(config.auto_indent);
        editor.set_passthrough(!config.vim_bindings);
        editor.set_tab_width(config.tab_width);
        editor.with_buffer_mut(|buffer| {
            buffer.set_wrap(if config.word_wrap {
                Wrap::Word
            } else {
                Wrap::None
            })
        });
        //TODO: dynamically discover light/dark changes
        editor.update_theme(config.syntax_theme());
    }

    pub fn open(&mut self, path: PathBuf) {
        let mut editor = self.editor.lock().unwrap();
        let mut font_system = FONT_SYSTEM.lock().unwrap();
        let mut editor = editor.borrow_with(&mut font_system);
        match editor.load_text(&path, self.attrs) {
            Ok(()) => {
                log::info!("opened {:?}", path);
                self.path_opt = match fs::canonicalize(&path) {
                    Ok(ok) => Some(ok),
                    Err(err) => {
                        log::error!("failed to canonicalize {:?}: {}", path, err);
                        Some(path)
                    }
                };
            }
            Err(err) => {
                log::error!("failed to open {:?}: {}", path, err);
                self.path_opt = None;
            }
        }
    }

    pub fn reload(&mut self) {
        let mut editor = self.editor.lock().unwrap();
        let mut font_system = FONT_SYSTEM.lock().unwrap();
        let mut editor = editor.borrow_with(&mut font_system);
        if let Some(path) = &self.path_opt {
            // Save scroll
            let scroll = editor.with_buffer(|buffer| buffer.scroll());
            //TODO: save/restore more?

            match editor.load_text(path, self.attrs) {
                Ok(()) => {
                    log::info!("reloaded {:?}", path);
                }
                Err(err) => {
                    log::error!("failed to reload {:?}: {}", path, err);
                }
            }

            // Restore scroll
            editor.with_buffer_mut(|buffer| buffer.set_scroll(scroll));
        } else {
            log::warn!("tried to reload with no path");
        }
    }

    pub fn save(&mut self) {
        if let Some(path) = &self.path_opt {
            let mut editor = self.editor.lock().unwrap();
            let mut text = String::new();
            editor.with_buffer(|buffer| {
                for line in buffer.lines.iter() {
                    text.push_str(line.text());
                    text.push('\n');
                }
            });
            match fs::write(path, text) {
                Ok(()) => {
                    editor.set_changed(false);
                    log::info!("saved {:?}", path);
                }
                Err(err) => {
                    log::error!("failed to save {:?}: {}", path, err);
                }
            }
        } else {
            log::warn!("tab has no path yet");
        }
    }

    pub fn watch(&self, watcher_opt: &mut Option<notify::RecommendedWatcher>) {
        if let Some(path) = &self.path_opt {
            if let Some(watcher) = watcher_opt {
                match watcher.watch(path, notify::RecursiveMode::NonRecursive) {
                    Ok(()) => {
                        log::info!("watching {:?} for changes", path);
                    }
                    Err(err) => {
                        log::warn!("failed to watch {:?} for changes: {:?}", path, err);
                    }
                }
            }
        }
    }

    pub fn changed(&self) -> bool {
        let editor = self.editor.lock().unwrap();
        editor.changed()
    }

    pub fn icon(&self, size: u16) -> Icon {
        match &self.path_opt {
            Some(path) => mime_icon(path, size),
            None => icon::from_name(FALLBACK_MIME_ICON).size(size).icon(),
        }
    }

    pub fn title(&self) -> String {
        //TODO: show full title when there is a conflict
        if let Some(path) = &self.path_opt {
            match path.file_name() {
                Some(file_name_os) => match file_name_os.to_str() {
                    Some(file_name) => file_name.to_string(),
                    None => format!("{}", path.display()),
                },
                None => format!("{}", path.display()),
            }
        } else {
            fl!("new-document")
        }
    }
}
