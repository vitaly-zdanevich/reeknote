use reeknote::config::Config;
use reeknote::edam_client::EdamClient;
use reeknote::editor::ImageOptions;
use reeknote::errors::{ReeknoteError, Result};
use reeknote::geeknote::{EvernoteClient, NotesService};
use reeknote::gnsync::{self, SyncFormat};
use reeknote::storage::Storage;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
struct SyncArgs {
    path: PathBuf,
    notebook: Option<String>,
    format: SyncFormat,
    count: usize,
    mask: Option<String>,
    all: bool,
    all_linked: bool,
    image_options: ImageOptions,
}

fn main() {
    let exit_code = match run(std::env::args().skip(1).collect()) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    };
    std::process::exit(exit_code);
}

fn run(args: Vec<String>) -> Result<()> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print!("{}", help());
        return Ok(());
    }
    let args = parse_args(args)?;
    let config = Config::load();
    let storage = Storage::open(config.app_dir.join("reeknote.db"))?;
    let token = auth_token(&storage)?;
    let mut client = EdamClient::new(token, &config);
    if args.all_linked {
        for notebook in client.find_linked_notebooks()? {
            let notebook_path = args
                .path
                .join(gnsync::escape_path_component(&notebook.share_name));
            let notes = client.download_linked_notebook_notes(
                &notebook,
                args.count,
                args.image_options.save_images,
            )?;
            for note in notes {
                let path = gnsync::create_file_from_note(
                    &note,
                    &notebook_path,
                    args.format.clone(),
                    &args.image_options,
                )?;
                println!("{}", path.display());
            }
        }
        return Ok(());
    }

    if args.all {
        for notebook in client.find_notebooks()? {
            let notebook_path = args
                .path
                .join(gnsync::escape_path_component(&notebook.name));
            download_notebook(&mut client, Some(&notebook.name), &notebook_path, &args)?;
        }
        return Ok(());
    }

    let notebook = args.notebook.as_deref().or_else(|| {
        args.path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
    });
    download_notebook(&mut client, notebook, &args.path, &args)
}

fn download_notebook(
    client: &mut EdamClient,
    notebook: Option<&str>,
    path: &PathBuf,
    args: &SyncArgs,
) -> Result<()> {
    let request =
        NotesService::create_search_request(None, &[], notebook, None, false, false, false, false)?;
    let result = client.find_notes(&request, args.count, false)?;
    for metadata in result.notes {
        let note = if args.image_options.save_images {
            client.get_note_with_resources(&metadata.guid)?
        } else {
            client.get_note(&metadata.guid)?
        };
        let path =
            gnsync::create_file_from_note(&note, path, args.format.clone(), &args.image_options)?;
        println!("{}", path.display());
    }
    Ok(())
}

fn parse_args(args: Vec<String>) -> Result<SyncArgs> {
    let mut path = PathBuf::from(".");
    let mut notebook = None;
    let mut format = SyncFormat::Plain;
    let mut count = 100usize;
    let mut mask = None;
    let mut all = false;
    let mut all_linked = false;
    let mut save_images = false;
    let mut images_in_subdir = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--path" | "-p" => {
                path = PathBuf::from(next_value(&mut iter, &arg)?);
            }
            "--mask" | "-m" => {
                mask = Some(next_value(&mut iter, &arg)?);
            }
            "--notebook" | "-n" => {
                notebook = Some(next_value(&mut iter, &arg)?);
            }
            "--format" | "-f" => {
                format = match next_value(&mut iter, &arg)?.as_str() {
                    "plain" => SyncFormat::Plain,
                    "markdown" => SyncFormat::Markdown,
                    "html" => SyncFormat::Html,
                    value => {
                        return Err(ReeknoteError::InvalidInput(format!(
                            "unsupported sync format: {value}"
                        )));
                    }
                };
            }
            "--count" => {
                count = next_value(&mut iter, &arg)?.parse().map_err(|_| {
                    ReeknoteError::InvalidInput("--count must be an integer".to_string())
                })?;
            }
            "--download-only" => {}
            "--all" | "-a" => {
                all = true;
            }
            "--save-images" => {
                save_images = true;
            }
            "--images-in-subdir" => {
                images_in_subdir = true;
            }
            "--logpath" | "-l" => {
                let _ = next_value(&mut iter, &arg)?;
            }
            "--sleep-on-ratelimit" => {}
            "--nodownsync" | "-d" => {
                return Err(ReeknoteError::Unsupported(
                    "nodownsync is not useful in download-only mode yet".to_string(),
                ));
            }
            "--two-way" => {
                return Err(ReeknoteError::Unsupported(
                    "two-way sync is not implemented in the Rust client yet".to_string(),
                ));
            }
            "--all-linked" => {
                all_linked = true;
            }
            other => {
                return Err(ReeknoteError::InvalidInput(format!(
                    "unexpected argument: {other}"
                )));
            }
        }
    }
    Ok(SyncArgs {
        path,
        notebook,
        format,
        count,
        mask,
        all,
        all_linked,
        image_options: ImageOptions {
            save_images,
            images_in_subdir,
            base_filename: None,
        },
    })
}

fn next_value(iter: &mut impl Iterator<Item = String>, arg: &str) -> Result<String> {
    iter.next()
        .ok_or_else(|| ReeknoteError::InvalidInput(format!("{arg} requires a value")))
}

fn auth_token(storage: &Storage) -> Result<String> {
    if let Ok(token) = std::env::var("EVERNOTE_DEV_TOKEN") {
        if !token.is_empty() {
            return Ok(token);
        }
    }
    storage.get_user_token().ok_or_else(|| {
        ReeknoteError::InvalidInput(
            "not logged in; run `geeknote login` or set EVERNOTE_DEV_TOKEN".to_string(),
        )
    })
}

fn help() -> String {
    "Usage: gnsync [--path PATH] [--notebook NOTEBOOK] [--all] [--all-linked] [--mask MASK] [--format plain|markdown|html] [--count N] [--download-only] [--save-images] [--images-in-subdir]\n\nThis Rust gnsync currently downloads notes to local files only. It does not create, update, or delete Evernote notes.\n".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_download_args() {
        let args = parse_args(vec![
            "--path".to_string(),
            "/tmp/notes".to_string(),
            "--notebook".to_string(),
            "Work".to_string(),
            "--format".to_string(),
            "markdown".to_string(),
            "--download-only".to_string(),
        ])
        .unwrap();
        assert_eq!(args.path, PathBuf::from("/tmp/notes"));
        assert_eq!(args.notebook.as_deref(), Some("Work"));
        assert_eq!(args.format, SyncFormat::Markdown);
        assert!(!args.all);
        assert!(!args.all_linked);
    }

    #[test]
    fn rejects_two_way_sync() {
        assert!(parse_args(vec!["--two-way".to_string()]).is_err());
    }

    #[test]
    fn parses_all_notebooks_and_image_options() {
        let args = parse_args(vec![
            "--all".to_string(),
            "--all-linked".to_string(),
            "--mask".to_string(),
            "*.md".to_string(),
            "--save-images".to_string(),
            "--images-in-subdir".to_string(),
        ])
        .unwrap();
        assert!(args.all);
        assert!(args.all_linked);
        assert_eq!(args.mask.as_deref(), Some("*.md"));
        assert!(args.image_options.save_images);
        assert!(args.image_options.images_in_subdir);
    }
}
