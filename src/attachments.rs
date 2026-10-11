//! Download note attachments without overwriting local files.

use crate::errors::{ReeknoteError, Result};
use crate::models::{Note, Resource};
use std::borrow::Cow;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

/// Save all attachments to `directory`, reporting each completed file to `output`.
///
/// `fetch` supplies bytes for resources whose bodies are not included in the note.
/// Returns the number of saved attachments, without exporting the note's text.
/// Files are published only after a complete write, with numeric suffixes for
/// existing names. On failure, previously completed attachments remain saved.
pub fn download_attachments(
    note: &Note,
    directory: &Path,
    mut fetch: impl FnMut(&Resource) -> Result<Vec<u8>>,
    mut output: impl Write,
) -> Result<usize> {
    for (index, resource) in note.resources.iter().enumerate() {
        let filename = attachment_filename(resource, index);
        let body = if resource.data.body.is_empty() {
            Cow::Owned(fetch(resource).map_err(|error| {
                ReeknoteError::External(format!("cannot download attachment {filename:?}: {error}"))
            })?)
        } else {
            Cow::Borrowed(resource.data.body.as_slice())
        };
        if resource.data.size > 0 && resource.data.size != body.len() {
            return Err(ReeknoteError::External(format!(
                "attachment {filename:?}: expected {} bytes, received {}",
                resource.data.size,
                body.len()
            )));
        }
        let mut file = NamedTempFile::new_in(directory)?;
        file.write_all(&body)?;
        file.flush()?;
        let path = persist_attachment(file, directory, &filename)?;
        writeln!(output, "{}", path.display())?;
        output.flush()?;
    }
    Ok(note.resources.len())
}

/// Keep the original basename, excluding path separators and terminal controls.
fn attachment_filename(resource: &Resource, index: usize) -> String {
    let filename = resource
        .filename
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default();
    let filename = filename
        .chars()
        .map(|character| {
            if character.is_control() {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    if !filename.trim().is_empty() && !matches!(filename.as_str(), "." | "..") {
        return filename;
    }
    let extension = resource
        .mime
        .as_deref()
        .and_then(mime_guess::get_mime_extensions_str)
        .and_then(|extensions| extensions.first().copied())
        .unwrap_or("bin");
    format!("attachment-{}.{extension}", index + 1)
}

/// Choose a free name atomically, including when another download races this one.
fn persist_attachment(
    mut file: NamedTempFile,
    directory: &Path,
    filename: &str,
) -> Result<PathBuf> {
    let name = Path::new(filename);
    let stem = name
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(filename);
    let extension = name.extension().and_then(|value| value.to_str());
    let mut suffix = 0u64;
    loop {
        let candidate = if suffix == 0 {
            filename.to_string()
        } else if let Some(extension) = extension {
            format!("{stem}-{suffix}.{extension}")
        } else {
            format!("{stem}-{suffix}")
        };
        let path = directory.join(candidate);
        match file.persist_noclobber(&path) {
            Ok(_) => return Ok(path),
            Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
                file = error.file;
                suffix += 1;
            }
            Err(error) => return Err(error.error.into()),
        }
    }
}
