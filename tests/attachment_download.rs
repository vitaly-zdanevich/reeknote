//! Fixture-based tests for attachment bytes, filenames, and safe local writes.

use reeknote::attachments::download_attachments;
use reeknote::errors::ReeknoteError;
use reeknote::models::{Note, Resource, ResourceData};
use std::fs;
use std::path::Path;

/// Construct a resource with realistic size and hash metadata.
fn resource(filename: &str, mime: &str, body: &[u8]) -> Resource {
    Resource {
        guid: format!("resource-{filename}"),
        filename: filename.to_string(),
        mime: Some(mime.to_string()),
        data: ResourceData {
            body: body.to_vec(),
            size: body.len(),
            body_hash: format!("{:x}", md5::compute(body)),
        },
    }
}

/// List only the final files; temporary downloads must never be left behind.
fn filenames(directory: &Path) -> Vec<String> {
    let mut names = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    names
}

/// All resource types are downloaded even when absent from the note's ENML.
#[test]
fn downloads_all_attachments_with_original_names_and_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let resources = vec![
        resource("photo.png", "image/png", b"\x89PNG\r\n\x1a\n\0image"),
        resource("voice recording.mp3", "audio/mpeg", b"ID3\0audio"),
        resource("caf\u{e9}.pdf", "application/pdf", b"%PDF-1.7\0document"),
        resource("archive.bin", "application/octet-stream", b"\0\x01\xff"),
    ];
    let note = Note {
        title: "Attachments".to_string(),
        content: "<en-note>Text only</en-note>".to_string(),
        resources: resources.clone(),
        ..Note::default()
    };
    let mut output = Vec::new();
    let count = download_attachments(
        &note,
        directory.path(),
        |_| panic!("already loaded"),
        &mut output,
    )
    .unwrap();
    assert_eq!(count, resources.len());
    assert_eq!(filenames(directory.path()).len(), resources.len());
    for resource in resources {
        let path = directory.path().join(&resource.filename);
        assert_eq!(fs::read(&path).unwrap(), resource.data.body);
        assert!(String::from_utf8_lossy(&output).contains(&path.display().to_string()));
    }
}

/// Missing bodies are retrieved once per resource, including empty attachments.
#[test]
fn fetches_missing_bodies_and_preserves_zero_byte_files() {
    let directory = tempfile::tempdir().unwrap();
    let mut remote = resource("remote.pdf", "application/pdf", b"remote data");
    remote.data.body.clear();
    let empty = resource("empty.txt", "text/plain", b"");
    let note = Note {
        resources: vec![remote.clone(), empty.clone()],
        ..Note::default()
    };
    let mut fetched = Vec::new();
    let count = download_attachments(
        &note,
        directory.path(),
        |resource| {
            fetched.push(resource.guid.clone());
            Ok(if resource.guid == remote.guid {
                b"remote data".to_vec()
            } else {
                Vec::new()
            })
        },
        Vec::new(),
    )
    .unwrap();
    assert_eq!(count, 2);
    assert_eq!(fetched, vec![remote.guid, empty.guid]);
    assert_eq!(
        fs::read(directory.path().join("remote.pdf")).unwrap(),
        b"remote data"
    );
    assert_eq!(fs::read(directory.path().join("empty.txt")).unwrap(), b"");
}

/// Repeat downloads and duplicate names must not replace existing local content.
#[test]
fn adds_suffixes_for_duplicate_names_existing_files_and_directories() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("report.pdf"), b"local document").unwrap();
    fs::create_dir(directory.path().join("report-1.pdf")).unwrap();
    let note = Note {
        resources: vec![
            resource("report.pdf", "application/pdf", b"first"),
            resource("report.pdf", "application/pdf", b"second"),
        ],
        ..Note::default()
    };
    assert_eq!(
        download_attachments(
            &note,
            directory.path(),
            |_| panic!("already loaded"),
            Vec::new()
        )
        .unwrap(),
        2
    );
    assert_eq!(
        fs::read(directory.path().join("report.pdf")).unwrap(),
        b"local document"
    );
    assert!(directory.path().join("report-1.pdf").is_dir());
    assert_eq!(
        fs::read(directory.path().join("report-2.pdf")).unwrap(),
        b"first"
    );
    assert_eq!(
        fs::read(directory.path().join("report-3.pdf")).unwrap(),
        b"second"
    );
    assert_eq!(filenames(directory.path()).len(), 4);
}

/// A note with no resources performs no network or filesystem writes.
#[test]
fn handles_notes_without_attachments() {
    let directory = tempfile::tempdir().unwrap();
    let mut output = Vec::new();
    assert_eq!(
        download_attachments(
            &Note::default(),
            directory.path(),
            |_| panic!("no resources"),
            &mut output
        )
        .unwrap(),
        0
    );
    assert!(output.is_empty());
    assert!(filenames(directory.path()).is_empty());
}

/// Remote paths and terminal control characters cannot escape the destination.
#[test]
fn sanitizes_paths_and_control_characters() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("downloads");
    fs::create_dir(&destination).unwrap();
    fs::write(directory.path().join("outside.txt"), b"outside").unwrap();
    let names = [
        ("../outside.txt", "outside.txt"),
        ("/absolute/photo.png", "photo.png"),
        ("C:\\private\\voice.mp3", "voice.mp3"),
        ("line\n\x1b[31m.txt", "line__[31m.txt"),
    ];
    let note = Note {
        resources: names
            .iter()
            .map(|(name, _)| resource(name, "application/octet-stream", b"attachment"))
            .collect(),
        ..Note::default()
    };
    download_attachments(
        &note,
        &destination,
        |_| panic!("already loaded"),
        Vec::new(),
    )
    .unwrap();
    for (_, expected) in names {
        assert_eq!(fs::read(destination.join(expected)).unwrap(), b"attachment");
    }
    assert_eq!(
        fs::read(directory.path().join("outside.txt")).unwrap(),
        b"outside"
    );
    assert_eq!(filenames(&destination).len(), names.len());
}

/// Unnamed or unusable filenames get an index and a MIME-derived extension.
#[test]
fn names_unnamed_attachments_without_trusting_mime_paths() {
    let directory = tempfile::tempdir().unwrap();
    let note = Note {
        resources: vec![
            resource("", "image/png", b"image"),
            resource("..", "application/pdf", b"pdf"),
            resource(" ", "invalid/../../outside", b"binary"),
            resource(".", "", b"unknown"),
        ],
        ..Note::default()
    };
    download_attachments(
        &note,
        directory.path(),
        |_| panic!("already loaded"),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        filenames(directory.path()),
        vec![
            "attachment-1.png",
            "attachment-2.pdf",
            "attachment-3.bin",
            "attachment-4.bin"
        ]
    );
}

/// Fetch failures retain completed files and remove the failing temporary file.
#[test]
fn reports_download_failure_without_creating_partial_files() {
    let directory = tempfile::tempdir().unwrap();
    let note = Note {
        resources: vec![
            resource("ready.txt", "text/plain", b"ready"),
            resource("failed.pdf", "application/pdf", b""),
        ],
        ..Note::default()
    };
    let mut output = Vec::new();
    let error = download_attachments(
        &note,
        directory.path(),
        |_| Err(ReeknoteError::External("network failure".to_string())),
        &mut output,
    )
    .unwrap_err();
    assert!(error.to_string().contains("network failure"));
    assert!(error.to_string().contains("failed.pdf"));
    assert_eq!(filenames(directory.path()), vec!["ready.txt"]);
    assert!(String::from_utf8_lossy(&output).contains("ready.txt"));
    assert!(!String::from_utf8_lossy(&output).contains("failed.pdf"));
}

/// A response with fewer bytes than advertised must not be saved as complete.
#[test]
fn rejects_incomplete_resource_data() {
    let directory = tempfile::tempdir().unwrap();
    let mut attachment = resource("truncated.pdf", "application/pdf", b"complete");
    attachment.data.body.clear();
    let note = Note {
        resources: vec![attachment],
        ..Note::default()
    };
    let error = download_attachments(
        &note,
        directory.path(),
        |_| Ok(b"short".to_vec()),
        Vec::new(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("truncated.pdf"));
    assert!(filenames(directory.path()).is_empty());
}

/// Nonexistent destinations fail without silently discarding attachments.
#[test]
fn reports_filesystem_errors() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("not-a-directory");
    fs::write(&destination, b"keep").unwrap();
    let note = Note {
        resources: vec![resource("file.txt", "text/plain", b"data")],
        ..Note::default()
    };
    assert!(matches!(
        download_attachments(
            &note,
            &destination,
            |_| panic!("already loaded"),
            Vec::new()
        ),
        Err(ReeknoteError::Io(_))
    ));
    assert_eq!(fs::read(destination).unwrap(), b"keep");
}

/// Existing symlinks, including dangling ones, are treated as name collisions.
#[cfg(unix)]
#[test]
fn never_follows_existing_symlinks() {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("downloads");
    fs::create_dir(&destination).unwrap();
    let outside = directory.path().join("outside.txt");
    let missing = directory.path().join("missing.txt");
    fs::write(&outside, b"outside").unwrap();
    symlink(&outside, destination.join("link.txt")).unwrap();
    symlink(&missing, destination.join("dangling.txt")).unwrap();
    let note = Note {
        resources: vec![
            resource("link.txt", "text/plain", b"one"),
            resource("dangling.txt", "text/plain", b"two"),
        ],
        ..Note::default()
    };
    download_attachments(
        &note,
        &destination,
        |_| panic!("already loaded"),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(fs::read(&outside).unwrap(), b"outside");
    assert!(!missing.exists());
    assert!(destination.join("link.txt").is_symlink());
    assert!(destination.join("dangling.txt").is_symlink());
    assert_eq!(fs::read(destination.join("link-1.txt")).unwrap(), b"one");
    assert_eq!(
        fs::read(destination.join("dangling-1.txt")).unwrap(),
        b"two"
    );
}

/// Two concurrent downloads must each retain their bytes under distinct names.
#[test]
fn concurrent_downloads_do_not_overwrite_each_other() {
    let directory = tempfile::tempdir().unwrap();
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        for body in [b"first".as_slice(), b"second".as_slice()] {
            let barrier = &barrier;
            let directory = directory.path();
            scope.spawn(move || {
                let mut remote = resource("shared.txt", "text/plain", body);
                remote.data.body.clear();
                let note = Note {
                    resources: vec![remote],
                    ..Note::default()
                };
                download_attachments(
                    &note,
                    directory,
                    |_| {
                        barrier.wait();
                        Ok(body.to_vec())
                    },
                    Vec::new(),
                )
                .unwrap();
            });
        }
    });
    assert_eq!(
        filenames(directory.path()),
        vec!["shared-1.txt", "shared.txt"]
    );
    let mut bodies = filenames(directory.path())
        .iter()
        .map(|name| fs::read(directory.path().join(name)).unwrap())
        .collect::<Vec<_>>();
    bodies.sort();
    assert_eq!(bodies, vec![b"first".to_vec(), b"second".to_vec()]);
}
