//! Typst's `World` over in-memory sources: one template file, `sys.inputs`, and fonts. Nothing reads the file system.

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Dict, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_kit::fonts::FontStore;

pub(crate) struct ResumeWorld {
    library: LazyHash<Library>,
    fonts: FontStore,
    source: Source,
}

impl ResumeWorld {
    pub(crate) fn new(template: &str, fonts: FontStore) -> Self {
        let main = RootedPath::new(
            VirtualRoot::Project,
            VirtualPath::new("/resume.typ").expect("valid path"),
        )
        .intern();
        Self {
            library: LazyHash::new(Library::default()),
            fonts,
            source: Source::new(main, template.to_owned()),
        }
    }

    /// Sets `sys.inputs` for the next compile. Everything else (parsed source, fonts) is kept, so Typst's memoization
    /// reuses what the inputs don't touch.
    pub(crate) fn set_inputs(&mut self, inputs: Dict) {
        self.library = LazyHash::new(Library::builder().with_inputs(inputs).build());
    }

    pub(crate) fn main_source(&self) -> &Source {
        &self.source
    }
}

impl World for ResumeWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.source.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.source.id() {
            Ok(self.source.clone())
        } else {
            Err(not_found(id))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.source.id() {
            Ok(Bytes::from_string(self.source.clone()))
        } else {
            Err(not_found(id))
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    /// No clock: a template's output depends only on its inputs.
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

fn not_found(id: FileId) -> FileError {
    FileError::NotFound(id.get().vpath().get_without_slash().into())
}
