use reeknote::argparser::{ArgParser, ArgValue, ParseOutcome, ParsedArgs};
use reeknote::config::Config;
use reeknote::edam_client::EdamClient;
use reeknote::editor;
use reeknote::errors::{ReeknoteError, Result};
use reeknote::models::{ListItem, Note, Notebook, Tag};
use reeknote::oauth::OAuthClient;
use reeknote::out;
use reeknote::reeknote as app;
use reeknote::reeknote::{EvernoteClient, NotesService};
use reeknote::storage::Storage;
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

const METADATA_CACHE_TTL_SECONDS: i64 = 3_600;
const NOTEBOOKS_CACHE_UPDATED_AT: &str = "notebooks_cache_updated_at";
const TAGS_CACHE_UPDATED_AT: &str = "tags_cache_updated_at";

fn main() {
    let exit_code = match run(std::env::args().skip(1).collect()) {
        Ok(()) => 0,
        Err(error) => {
            out::failure_message(&error.to_string());
            1
        }
    };
    std::process::exit(exit_code);
}

fn run(args: Vec<String>) -> Result<()> {
    let parser = ArgParser::default();
    let command = args.first().cloned();
    let outcome = parser.parse(args).map_err(|error| {
        ReeknoteError::InvalidInput(format!("{}\n{}", error.message, error.help))
    })?;

    let ParseOutcome::Parsed(mut values) = outcome else {
        match outcome {
            ParseOutcome::About => print!("{}", out::about()),
            ParseOutcome::Help(help) => print!("{help}"),
            ParseOutcome::Parsed(_) => unreachable!(),
        }
        return Ok(());
    };

    if matches!(values.get("content"), Some(ArgValue::String(value)) if value == "-") {
        let mut content = String::new();
        io::stdin().read_to_string(&mut content)?;
        values.insert("content".to_string(), ArgValue::String(content));
    }

    let config = Config::load();
    let db_path = config.app_dir.join("reeknote.db");
    let mut storage = Storage::open(db_path)?;

    match command.as_deref() {
        Some("settings") => handle_settings(&mut storage, &config, values),
        Some("login") => handle_login(&mut storage, &config),
        Some("logout") => handle_logout(&mut storage, values),
        Some("user") => handle_user(&mut storage, &config, values),
        Some("find") => handle_find(&mut storage, &config, values),
        Some("show") => handle_show(&mut storage, &config, values),
        Some("create") => handle_create(&mut storage, &config, values),
        Some("create-linked") => handle_create_linked(&mut storage, &config, values),
        Some("edit") => handle_edit(&mut storage, &config, values),
        Some("edit-linked") => handle_edit_linked(&mut storage, &config, values),
        Some("remove") => handle_remove(&mut storage, &config, values),
        Some("dedup") => handle_dedup(&mut storage, &config, values),
        Some("notebook-list") => handle_notebook_list(&mut storage, &config, values),
        Some("notebook-create") => handle_notebook_create(&mut storage, &config, values),
        Some("notebook-edit") => handle_notebook_edit(&mut storage, &config, values),
        Some("notebook-remove") => handle_notebook_remove(&mut storage, &config, values),
        Some("tag-list") => handle_tag_list(&mut storage, &config, values),
        Some("tag-create") => handle_tag_create(&mut storage, &config, values),
        Some("tag-edit") => handle_tag_edit(&mut storage, &config, values),
        Some("tag-remove") => handle_tag_remove(&mut storage, &config, values),
        Some(other) => Err(ReeknoteError::InvalidInput(format!(
            "unknown command: {other}"
        ))),
        None => {
            print!("{}", out::about());
            Ok(())
        }
    }
}

fn handle_login(storage: &mut Storage, config: &Config) -> Result<()> {
    let token = if let Ok(token) = std::env::var("EVERNOTE_DEV_TOKEN") {
        token
    } else {
        print!("Developer token (leave empty to use OAuth): ");
        io::stdout().flush()?;
        let mut token = String::new();
        io::stdin().read_line(&mut token)?;
        let token = token.trim().to_string();
        if token.is_empty() {
            oauth_login(config)?
        } else {
            token
        }
    };

    if token.is_empty() {
        return Err(ReeknoteError::InvalidInput(
            "developer token is required; set EVERNOTE_DEV_TOKEN or enter one at the prompt"
                .to_string(),
        ));
    }

    let mut client = EdamClient::new(token.clone(), config);
    if !client.check_version()? {
        return Err(ReeknoteError::External(
            "Evernote EDAM protocol version is not compatible".to_string(),
        ));
    }
    let user = client.get_user_info()?;
    storage.create_user(token, user)?;
    println!("You have successfully logged in.");
    Ok(())
}

fn oauth_login(config: &Config) -> Result<String> {
    let oauth = OAuthClient::new(config);
    let callback = format!("https://{}", config.user_base_url);
    let request_token = oauth.request_token(&callback)?;
    println!(
        "Open this URL in your browser and approve access:\n{}",
        oauth.authorization_url(&request_token.token)
    );
    print!("Paste the oauth_verifier or final redirected URL: ");
    io::stdout().flush()?;
    let mut verifier = String::new();
    io::stdin().read_line(&mut verifier)?;
    let access_token = oauth.access_token(&request_token, &verifier)?;
    Ok(access_token.token)
}

fn handle_logout(storage: &mut Storage, values: ParsedArgs) -> Result<()> {
    if !arg_bool(&values, "force") && !confirm("Are you sure you want to logout?")? {
        return Ok(());
    }
    storage.remove_user()?;
    println!("You have successfully logged out.");
    Ok(())
}

fn handle_user(storage: &mut Storage, config: &Config, values: ParsedArgs) -> Result<()> {
    let full = arg_bool(&values, "full");
    let user = if full {
        make_client(storage, config)?.get_user_info()?
    } else {
        storage.get_user_info().ok_or_else(login_required)?
    };
    print!("{}", out::show_user(&user, full));
    Ok(())
}

fn handle_settings(storage: &mut Storage, config: &Config, values: ParsedArgs) -> Result<()> {
    let mut changed = false;

    if let Some(value) = values.get("editor") {
        match value {
            ArgValue::String(value) if value == "#GET#" => {
                println!("Current editor is: {}", app::get_editor(Some(storage)));
                return Ok(());
            }
            ArgValue::String(value) => {
                app::set_editor(storage, value)?;
                changed = true;
            }
            _ => {}
        }
    }

    if let Some(value) = values.get("extras") {
        match value {
            ArgValue::String(value) if value == "#GET#" => {
                println!(
                    "Current markdown2 extras is : {:?}",
                    app::get_extras(Some(storage))
                );
                return Ok(());
            }
            ArgValue::String(value) => {
                app::set_extras(storage, value)?;
                changed = true;
            }
            _ => {}
        }
    }

    if let Some(value) = values.get("note_ext") {
        match value {
            ArgValue::String(value) if value == "#GET#" => {
                println!(
                    "Current note extension is: {:?}",
                    app::get_note_ext(storage)
                );
                return Ok(());
            }
            ArgValue::String(value) => {
                app::set_note_ext(storage, value)?;
                changed = true;
            }
            _ => {}
        }
    }

    if changed {
        println!("Changes saved.");
    } else {
        println!("{}", app::settings_output(storage, config));
    }
    Ok(())
}

fn handle_find(storage: &mut Storage, config: &Config, values: ParsedArgs) -> Result<()> {
    let tags = arg_list(&values, "tag");
    let request = NotesService::create_search_request(
        arg_string(&values, "search").as_deref(),
        &tags,
        arg_string(&values, "notebook").as_deref(),
        arg_string(&values, "date").as_deref(),
        arg_bool(&values, "exact_entry"),
        arg_bool(&values, "content_search"),
        arg_bool(&values, "ignore_completed"),
        arg_bool(&values, "reminders_only"),
    )?;
    let count = arg_int(&values, "count").unwrap_or(20).max(1) as usize;
    let mut client = make_client(storage, config)?;
    let mut result = client.find_notes(&request, count, arg_bool(&values, "deleted_only"))?;
    if arg_bool(&values, "with_notebook") {
        let notebooks = client.find_notebooks()?;
        for note in &mut result.notes {
            if let Some(guid) = &note.notebook_guid {
                note.notebook_name = notebooks
                    .iter()
                    .find(|notebook| &notebook.guid == guid)
                    .map(|notebook| notebook.name.clone());
            }
        }
    }
    if arg_bool(&values, "with_tags") {
        hydrate_note_list_tag_names(storage, &mut client, &mut result.notes)?;
    }
    for note in &result.notes {
        storage.set_note(note.clone())?;
    }
    storage.set_search(result.clone())?;
    let items = result
        .notes
        .into_iter()
        .map(ListItem::Note)
        .collect::<Vec<_>>();
    print!(
        "{}",
        out::search_result(
            &items,
            &request,
            out::ListOptions {
                show_url: arg_bool(&values, "with_url"),
                show_tags: arg_bool(&values, "with_tags"),
                show_notebook: arg_bool(&values, "with_notebook"),
                show_guid: arg_bool(&values, "guid"),
                ..out::ListOptions::default()
            },
            config,
        )
    );
    Ok(())
}

fn handle_show(storage: &mut Storage, config: &Config, values: ParsedArgs) -> Result<()> {
    let note_ref = required_string(&values, "note")?;
    let mut client = make_client(storage, config)?;
    let raw = arg_bool(&values, "raw");
    let animate = !raw;
    let animate_resolve = animate && note_ref_resolves_without_selection(storage, &note_ref);
    let mut note = if animate_resolve {
        out::with_terminal_animation("Loading note", true, || {
            resolve_note(storage, &mut client, config, &note_ref)
        })?
    } else {
        resolve_note(storage, &mut client, config, &note_ref)?
    };

    let mut user = None;
    out::with_terminal_animation("Loading note", animate, || -> Result<()> {
        if note.content.is_empty() {
            note.content = client.get_note_content(&note.guid)?;
        }

        if !raw {
            hydrate_show_metadata(storage, &mut client, &mut note)?;
            user = Some(
                storage
                    .get_user_info()
                    .unwrap_or_else(|| client.get_user_info().unwrap_or_default()),
            );
        }
        Ok(())
    })?;

    if raw {
        print!("{}", note.content);
    } else {
        let user = user.unwrap_or_default();
        print!("{}", out::show_note(&note, user.id, &user.shard_id, config));
    }
    Ok(())
}

fn handle_create(storage: &mut Storage, config: &Config, values: ParsedArgs) -> Result<()> {
    let title = required_string(&values, "title")?;
    let raw = arg_bool(&values, "raw");
    let mut content = arg_string(&values, "content").unwrap_or_else(|| "WRITE".to_string());
    if content == "WRITE" {
        content = edit_note_text(storage, "", raw)?.content;
    }
    let mut client = make_client(storage, config)?;
    let notebook = if let Some(notebook) = arg_string(&values, "notebook") {
        Some(resolve_notebook_guid(&mut client, &notebook)?)
    } else {
        None
    };
    let mut input = NotesService::parse_input(
        Some(title),
        Some(content.clone()),
        arg_list(&values, "tag"),
        arg_string(&values, "created"),
        notebook,
        arg_list(&values, "resource"),
        None,
        arg_string(&values, "reminder"),
        arg_string(&values, "url"),
        arg_bool(&values, "rawmd"),
    )?;
    if raw {
        input.content = Some(content);
    }
    client.create_note(input)?;
    println!("Note successfully created.");
    Ok(())
}

fn handle_create_linked(storage: &mut Storage, config: &Config, values: ParsedArgs) -> Result<()> {
    let mut client = make_client(storage, config)?;
    client.create_linked_note(
        &required_string(&values, "notebook")?,
        &required_string(&values, "title")?,
    )?;
    println!("Linked note successfully created.");
    Ok(())
}

fn handle_edit(storage: &mut Storage, config: &Config, values: ParsedArgs) -> Result<()> {
    let note_ref = required_string(&values, "note")?;
    let mut client = make_client(storage, config)?;
    let mut note = resolve_note(storage, &mut client, config, &note_ref)?;
    let notebook = if let Some(notebook) = arg_string(&values, "notebook") {
        Some(resolve_notebook_guid(&mut client, &notebook)?)
    } else {
        None
    };
    let raw = arg_bool(&values, "raw");
    let mut raw_content = arg_string(&values, "content");
    if raw_content.is_none()
        && arg_string(&values, "title").is_none()
        && arg_list(&values, "tag").is_empty()
        && arg_string(&values, "created").is_none()
        && notebook.is_none()
        && arg_list(&values, "resource").is_empty()
        && arg_string(&values, "reminder").is_none()
        && arg_string(&values, "url").is_none()
    {
        if note.content.is_empty() {
            note.content = client.get_note_content(&note.guid)?;
        }
        let initial_content = if raw {
            note.content.clone()
        } else {
            editor::enml_to_text(&note.content)
        };
        let edited = edit_note_text(storage, &initial_content, raw)?;
        if !edited.changed {
            println!("Note was not changed.");
            return Ok(());
        }
        raw_content = Some(edited.content);
    }
    let mut input = NotesService::parse_input(
        arg_string(&values, "title"),
        raw_content.clone(),
        arg_list(&values, "tag"),
        arg_string(&values, "created"),
        notebook,
        arg_list(&values, "resource"),
        Some(&note),
        arg_string(&values, "reminder"),
        arg_string(&values, "url"),
        arg_bool(&values, "rawmd"),
    )?;
    if raw && let Some(content) = raw_content {
        input.content = Some(content);
    }
    client.update_note(&note.guid, input)?;
    println!("Note successfully saved.");
    Ok(())
}

fn handle_edit_linked(storage: &mut Storage, config: &Config, values: ParsedArgs) -> Result<()> {
    let notebook_ref = required_string(&values, "notebook")?;
    let note_ref = required_string(&values, "note")?;
    let mut client = make_client(storage, config)?;
    let note = client.find_linked_note(&notebook_ref, &note_ref)?;
    let edited = edit_note_text(storage, &editor::enml_to_text(&note.content), false)?;
    if !edited.changed {
        println!("Note was not changed.");
        return Ok(());
    }
    client.update_linked_note_content(
        &notebook_ref,
        &note,
        editor::text_to_enml(&edited.content),
    )?;
    println!("Linked note successfully saved.");
    Ok(())
}

fn handle_remove(storage: &mut Storage, config: &Config, values: ParsedArgs) -> Result<()> {
    let note_ref = required_string(&values, "note")?;
    let mut client = make_client(storage, config)?;
    let note = resolve_note(storage, &mut client, config, &note_ref)?;
    if !arg_bool(&values, "force")
        && !confirm(&format!(
            "Are you sure you want to delete this note: \"{}\"?",
            note.title
        ))?
    {
        return Ok(());
    }
    client.remove_note(&note.guid)?;
    println!("Note successfully deleted.");
    Ok(())
}

fn handle_dedup(storage: &mut Storage, config: &Config, values: ParsedArgs) -> Result<()> {
    let request = NotesService::create_search_request(
        None,
        &[],
        arg_string(&values, "notebook").as_deref(),
        None,
        false,
        false,
        false,
        false,
    )?;
    let mut client = make_client(storage, config)?;
    let result = client.find_notes(&request, 10_000, false)?;
    let candidate_groups = app::duplicate_metadata_groups(&result.notes);
    let mut candidates = Vec::new();
    for metadata in candidate_groups.into_iter().flatten() {
        candidates.push(client.get_note(&metadata.guid)?);
    }
    let duplicate_groups = app::duplicate_content_groups(candidates);
    print!(
        "{}",
        out::dedup_preview(&duplicate_groups, result.notes.len())
    );
    Ok(())
}

fn handle_notebook_list(storage: &mut Storage, config: &Config, values: ParsedArgs) -> Result<()> {
    let mut client = make_client(storage, config)?;
    let notebooks = client.find_notebooks()?;
    cache_notebooks(storage, &notebooks)?;
    let mut items = notebooks
        .into_iter()
        .map(ListItem::Notebook)
        .collect::<Vec<_>>();
    items.extend(
        client
            .find_linked_notebooks()?
            .into_iter()
            .map(ListItem::LinkedNotebook),
    );
    print!(
        "{}",
        out::print_list(
            &items,
            "",
            out::ListOptions {
                show_guid: arg_bool(&values, "guid"),
                ..out::ListOptions::default()
            },
            config,
        )
    );
    Ok(())
}

fn handle_notebook_create(
    storage: &mut Storage,
    config: &Config,
    values: ParsedArgs,
) -> Result<()> {
    let mut client = make_client(storage, config)?;
    client.create_notebook(
        &required_string(&values, "title")?,
        arg_string(&values, "stack").as_deref(),
    )?;
    println!("Notebook successfully created.");
    Ok(())
}

fn handle_notebook_edit(storage: &mut Storage, config: &Config, values: ParsedArgs) -> Result<()> {
    let mut client = make_client(storage, config)?;
    let notebook = resolve_notebook_guid(&mut client, &required_string(&values, "notebook")?)?;
    client.update_notebook(&notebook, &required_string(&values, "title")?)?;
    println!("Notebook successfully updated.");
    Ok(())
}

fn handle_notebook_remove(
    storage: &mut Storage,
    config: &Config,
    values: ParsedArgs,
) -> Result<()> {
    let mut client = make_client(storage, config)?;
    let notebook_ref = required_string(&values, "notebook")?;
    let notebook = resolve_notebook_guid(&mut client, &notebook_ref)?;
    if !arg_bool(&values, "force")
        && !confirm(&format!(
            "Are you sure you want to delete this notebook: \"{notebook_ref}\"?"
        ))?
    {
        return Ok(());
    }
    client.remove_notebook(&notebook)?;
    println!("Notebook successfully removed.");
    Ok(())
}

fn handle_tag_list(storage: &mut Storage, config: &Config, values: ParsedArgs) -> Result<()> {
    let mut client = make_client(storage, config)?;
    let tags = client.find_tags()?;
    cache_tags(storage, &tags)?;
    let items = tags.into_iter().map(ListItem::Tag).collect::<Vec<_>>();
    print!(
        "{}",
        out::print_list(
            &items,
            "",
            out::ListOptions {
                show_guid: arg_bool(&values, "guid"),
                ..out::ListOptions::default()
            },
            config,
        )
    );
    Ok(())
}

fn handle_tag_create(storage: &mut Storage, config: &Config, values: ParsedArgs) -> Result<()> {
    let mut client = make_client(storage, config)?;
    client.create_tag(&required_string(&values, "title")?)?;
    println!("Tag successfully created.");
    Ok(())
}

fn handle_tag_edit(storage: &mut Storage, config: &Config, values: ParsedArgs) -> Result<()> {
    let mut client = make_client(storage, config)?;
    let tag = resolve_tag_guid(&mut client, &required_string(&values, "tagname")?)?;
    client.update_tag(&tag, &required_string(&values, "title")?)?;
    println!("Tag successfully updated.");
    Ok(())
}

fn handle_tag_remove(storage: &mut Storage, config: &Config, values: ParsedArgs) -> Result<()> {
    let mut client = make_client(storage, config)?;
    let tag_ref = required_string(&values, "tagname")?;
    let tag = resolve_tag_guid(&mut client, &tag_ref)?;
    if !arg_bool(&values, "force")
        && !confirm(&format!(
            "Are you sure you want to delete the tag \"{tag_ref}\"?"
        ))?
    {
        return Ok(());
    }
    client.remove_tag(&tag)?;
    println!("Tag successfully removed.");
    Ok(())
}

fn make_client(storage: &Storage, config: &Config) -> Result<EdamClient> {
    Ok(EdamClient::new(auth_token(storage)?, config))
}

fn edit_note_text(
    storage: &mut Storage,
    initial_content: &str,
    raw: bool,
) -> Result<editor::EditOutcome> {
    let editor_command = app::get_editor(Some(storage));
    let note_ext = app::get_note_ext(storage);
    let suffix = &note_ext[usize::from(raw)];
    editor::edit_content(&editor_command, initial_content, suffix)
}

fn auth_token(storage: &Storage) -> Result<String> {
    if let Ok(token) = std::env::var("EVERNOTE_DEV_TOKEN")
        && !token.is_empty()
    {
        return Ok(token);
    }
    storage.get_user_token().ok_or_else(login_required)
}

fn login_required() -> ReeknoteError {
    ReeknoteError::InvalidInput(
        "not logged in; run `reeknote login` or set EVERNOTE_DEV_TOKEN".to_string(),
    )
}

fn resolve_note(
    storage: &mut Storage,
    client: &mut EdamClient,
    config: &Config,
    note_ref: &str,
) -> Result<Note> {
    if let Some(note) = storage.get_note(note_ref) {
        return get_note_with_cached_metadata(storage, client, &note.guid);
    }

    if let Some(note) = storage.get_search().and_then(|search| {
        note_ref
            .parse::<usize>()
            .ok()
            .and_then(|index| search.notes.get(index.saturating_sub(1)).cloned())
    }) {
        return get_note_with_cached_metadata(storage, client, &note.guid);
    }

    if looks_like_guid(note_ref) {
        return get_note_with_cached_metadata(storage, client, note_ref);
    }

    let request = NotesService::create_search_request(
        Some(note_ref),
        &[],
        None,
        None,
        false,
        false,
        false,
        false,
    )?;
    let result = client.find_notes(&request, 20, false)?;
    match result.notes.as_slice() {
        [] => Err(ReeknoteError::InvalidInput(
            "notes have not been found".to_string(),
        )),
        [note] => {
            storage.set_note(note.clone())?;
            get_note_with_cached_metadata(storage, client, &note.guid)
        }
        _ => {
            for note in &result.notes {
                storage.set_note(note.clone())?;
            }
            storage.set_search(result.clone())?;
            let selected = select_note(&result.notes, config)?;
            get_note_with_cached_metadata(storage, client, &selected.guid)
        }
    }
}

fn get_note_with_cached_metadata(
    storage: &Storage,
    client: &mut EdamClient,
    guid: &str,
) -> Result<Note> {
    let cached = storage.get_note(guid);
    let mut note = client.get_note(guid)?;
    if let Some(cached) = cached {
        if note.tag_names.is_empty() && cache_is_fresh(storage, TAGS_CACHE_UPDATED_AT) {
            note.tag_names = cached.tag_names;
        }
        if note.notebook_name.as_deref().unwrap_or_default().is_empty()
            && cache_is_fresh(storage, NOTEBOOKS_CACHE_UPDATED_AT)
        {
            note.notebook_name = cached.notebook_name;
        }
    }
    Ok(note)
}

fn note_ref_resolves_without_selection(storage: &Storage, note_ref: &str) -> bool {
    if storage.get_note(note_ref).is_some() {
        return true;
    }
    if storage
        .get_search()
        .and_then(|search| {
            note_ref
                .parse::<usize>()
                .ok()
                .and_then(|index| search.notes.get(index.saturating_sub(1)).cloned())
        })
        .is_some()
    {
        return true;
    }
    looks_like_guid(note_ref)
}

fn select_note(notes: &[Note], config: &Config) -> Result<Note> {
    let items = notes
        .iter()
        .cloned()
        .map(ListItem::Note)
        .collect::<Vec<_>>();
    print!(
        "{}",
        out::print_list(
            &items,
            "",
            out::ListOptions {
                show_selector: true,
                ..out::ListOptions::default()
            },
            config,
        )
    );

    loop {
        print!(": ");
        io::stdout().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        match parse_note_selection(&answer, notes.len()) {
            Ok(Some(index)) => return Ok(notes[index].clone()),
            Ok(None) => {
                return Err(ReeknoteError::InvalidInput(
                    "note selection cancelled".to_string(),
                ));
            }
            Err(error) => out::failure_message(&format!("{error}, please try again.")),
        }
    }
}

fn parse_note_selection(input: &str, total: usize) -> Result<Option<usize>> {
    let input = input.trim();
    if input == "0" || input.eq_ignore_ascii_case("q") {
        return Ok(None);
    }
    let selection = input
        .parse::<usize>()
        .map_err(|_| ReeknoteError::InvalidInput(format!("incorrect number \"{input}\"")))?;
    if (1..=total).contains(&selection) {
        return Ok(Some(selection - 1));
    }
    Err(ReeknoteError::InvalidInput(format!(
        "incorrect number \"{input}\""
    )))
}

fn resolve_notebook_guid(client: &mut EdamClient, notebook_ref: &str) -> Result<String> {
    if looks_like_guid(notebook_ref) {
        return Ok(notebook_ref.to_string());
    }
    client
        .find_notebooks()?
        .into_iter()
        .find(|notebook| notebook.name == notebook_ref)
        .map(|notebook| notebook.guid)
        .ok_or_else(|| ReeknoteError::InvalidInput(format!("notebook not found: {notebook_ref}")))
}

fn resolve_tag_guid(client: &mut EdamClient, tag_ref: &str) -> Result<String> {
    if looks_like_guid(tag_ref) {
        return Ok(tag_ref.to_string());
    }
    client
        .find_tags()?
        .into_iter()
        .find(|tag| tag.name == tag_ref)
        .map(|tag| tag.guid)
        .ok_or_else(|| ReeknoteError::InvalidInput(format!("tag not found: {tag_ref}")))
}

fn looks_like_guid(value: &str) -> bool {
    value.len() == 36 && value.chars().filter(|character| *character == '-').count() == 4
}

fn hydrate_show_metadata(
    storage: &mut Storage,
    client: &mut EdamClient,
    note: &mut Note,
) -> Result<()> {
    if should_fetch_note_tag_names(note) {
        hydrate_note_tag_names(storage, client, note)?;
    }
    if should_fetch_note_notebook_name(note) {
        hydrate_note_notebook_name(storage, client, note)?;
    }
    storage.set_note(note.clone())?;
    Ok(())
}

fn hydrate_note_tag_names(
    storage: &mut Storage,
    client: &mut EdamClient,
    note: &mut Note,
) -> Result<()> {
    if let Some(tag_names) = cached_note_tag_names(storage, note) {
        note.tag_names = tag_names;
        return Ok(());
    }
    if let Some(tag_names) = cached_tag_names(storage, note) {
        note.tag_names = tag_names;
        return Ok(());
    }

    note.tag_names = client.get_note_tag_names(&note.guid)?;
    if !note.tag_guids.is_empty() && note.tag_guids.len() == note.tag_names.len() {
        let mut tags = storage.get_tags();
        for (guid, name) in note.tag_guids.iter().zip(&note.tag_names) {
            tags.insert(guid.clone(), name.clone());
        }
        storage.set_tags(tags)?;
    }
    mark_cache_updated(storage, TAGS_CACHE_UPDATED_AT)
}

fn hydrate_note_notebook_name(
    storage: &mut Storage,
    client: &mut EdamClient,
    note: &mut Note,
) -> Result<()> {
    if let Some(notebook_name) = cached_notebook_name(storage, note) {
        note.notebook_name = Some(notebook_name);
        return Ok(());
    }

    let notebooks = client.find_notebooks()?;
    cache_notebooks(storage, &notebooks)?;
    if let Some(notebook_name) = note_notebook_name(note, &notebooks) {
        note.notebook_name = Some(notebook_name);
    }
    Ok(())
}

fn should_fetch_note_tag_names(note: &Note) -> bool {
    note.tag_names.is_empty()
}

fn should_fetch_note_notebook_name(note: &Note) -> bool {
    note.notebook_name.as_deref().unwrap_or_default().is_empty()
        && note
            .notebook_guid
            .as_deref()
            .is_some_and(|guid| !guid.is_empty())
}

fn note_notebook_name(note: &Note, notebooks: &[Notebook]) -> Option<String> {
    let guid = note.notebook_guid.as_deref()?;
    notebooks
        .iter()
        .find(|notebook| notebook.guid == guid)
        .map(|notebook| notebook.name.clone())
}

fn cached_note_tag_names(storage: &Storage, note: &Note) -> Option<Vec<String>> {
    if cache_is_fresh(storage, TAGS_CACHE_UPDATED_AT) {
        let tag_names = storage.get_note(&note.guid)?.tag_names;
        if !tag_names.is_empty() {
            return Some(tag_names);
        }
    }
    None
}

fn cached_tag_names(storage: &Storage, note: &Note) -> Option<Vec<String>> {
    if note.tag_guids.is_empty() || !cache_is_fresh(storage, TAGS_CACHE_UPDATED_AT) {
        return None;
    }
    let tags = storage.get_tags();
    note.tag_guids
        .iter()
        .map(|guid| tags.get(guid).cloned())
        .collect()
}

fn hydrate_note_list_tag_names(
    storage: &mut Storage,
    client: &mut EdamClient,
    notes: &mut [Note],
) -> Result<()> {
    if !notes_need_tag_name_lookup(notes) {
        return Ok(());
    }

    if cache_is_fresh(storage, TAGS_CACHE_UPDATED_AT) {
        let tags = storage.get_tags();
        apply_tag_names_to_notes(notes, &tags);
        if !notes_need_tag_name_lookup(notes) {
            return Ok(());
        }
    }

    let tags = client.find_tags()?;
    cache_tags(storage, &tags)?;
    let tags = storage.get_tags();
    apply_tag_names_to_notes(notes, &tags);
    Ok(())
}

fn notes_need_tag_name_lookup(notes: &[Note]) -> bool {
    notes
        .iter()
        .any(|note| note.tag_guids.len() > note.tag_names.len())
}

fn apply_tag_names_to_notes(notes: &mut [Note], tags: &BTreeMap<String, String>) {
    for note in notes
        .iter_mut()
        .filter(|note| note.tag_guids.len() > note.tag_names.len())
    {
        let tag_names = note
            .tag_guids
            .iter()
            .filter_map(|guid| tags.get(guid).cloned())
            .collect::<Vec<_>>();
        if !tag_names.is_empty() {
            note.tag_names = tag_names;
        }
    }
}

fn cached_notebook_name(storage: &Storage, note: &Note) -> Option<String> {
    if cache_is_fresh(storage, NOTEBOOKS_CACHE_UPDATED_AT) {
        return note
            .notebook_guid
            .as_deref()
            .and_then(|guid| storage.get_notebooks().get(guid).cloned());
    }
    None
}

fn cache_tags(storage: &mut Storage, tags: &[Tag]) -> Result<()> {
    storage.set_tags(
        tags.iter()
            .map(|tag| (tag.guid.clone(), tag.name.clone()))
            .collect::<BTreeMap<_, _>>(),
    )?;
    mark_cache_updated(storage, TAGS_CACHE_UPDATED_AT)
}

fn cache_notebooks(storage: &mut Storage, notebooks: &[Notebook]) -> Result<()> {
    storage.set_notebooks(
        notebooks
            .iter()
            .map(|notebook| (notebook.guid.clone(), notebook.name.clone()))
            .collect::<BTreeMap<_, _>>(),
    )?;
    mark_cache_updated(storage, NOTEBOOKS_CACHE_UPDATED_AT)
}

fn cache_is_fresh(storage: &Storage, key: &str) -> bool {
    storage
        .get_setting(key)
        .and_then(|value| value.parse::<i64>().ok())
        .is_some_and(|updated_at| {
            current_timestamp_seconds() - updated_at <= METADATA_CACHE_TTL_SECONDS
        })
}

fn mark_cache_updated(storage: &mut Storage, key: &str) -> Result<()> {
    storage.set_setting(key, current_timestamp_seconds().to_string())
}

fn current_timestamp_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn confirm(message: &str) -> Result<bool> {
    println!("{message}");
    print!("Yes/No: ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_lowercase().as_str(),
        "yes" | "ye" | "y"
    ))
}

fn arg_string(values: &ParsedArgs, key: &str) -> Option<String> {
    values.get(key).and_then(|value| match value {
        ArgValue::String(value) => Some(value.clone()),
        _ => None,
    })
}

fn required_string(values: &ParsedArgs, key: &str) -> Result<String> {
    arg_string(values, key)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ReeknoteError::InvalidInput(format!("missing required argument: {key}")))
}

fn arg_list(values: &ParsedArgs, key: &str) -> Vec<String> {
    values
        .get(key)
        .and_then(|value| match value {
            ArgValue::List(values) => Some(values.clone()),
            ArgValue::String(value) => Some(vec![value.clone()]),
            _ => None,
        })
        .unwrap_or_default()
}

fn arg_bool(values: &ParsedArgs, key: &str) -> bool {
    matches!(values.get(key), Some(ArgValue::Bool(true)))
}

fn arg_int(values: &ParsedArgs, key: &str) -> Option<i64> {
    values.get(key).and_then(|value| match value {
        ArgValue::Int(value) => Some(*value),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_note_selection_numbers() {
        assert_eq!(parse_note_selection("1", 3).unwrap(), Some(0));
        assert_eq!(parse_note_selection("3", 3).unwrap(), Some(2));
    }

    #[test]
    fn parses_note_selection_cancel() {
        assert_eq!(parse_note_selection("0", 3).unwrap(), None);
        assert_eq!(parse_note_selection("q", 3).unwrap(), None);
    }

    #[test]
    fn rejects_invalid_note_selection() {
        assert!(parse_note_selection("4", 3).is_err());
        assert!(parse_note_selection("abc", 3).is_err());
    }

    #[test]
    fn fetches_tag_names_even_without_cached_tag_guids() {
        let note = Note {
            tag_names: Vec::new(),
            tag_guids: Vec::new(),
            ..Note::default()
        };
        assert!(should_fetch_note_tag_names(&note));
    }

    #[test]
    fn keeps_existing_tag_names() {
        let note = Note {
            tag_names: vec!["tag".to_string()],
            ..Note::default()
        };
        assert!(!should_fetch_note_tag_names(&note));
    }

    #[test]
    fn fetches_notebook_name_when_guid_is_known() {
        let note = Note {
            notebook_guid: Some("nb-guid".to_string()),
            notebook_name: None,
            ..Note::default()
        };
        assert!(should_fetch_note_notebook_name(&note));
    }

    #[test]
    fn keeps_existing_notebook_name() {
        let note = Note {
            notebook_guid: Some("nb-guid".to_string()),
            notebook_name: Some("Inbox".to_string()),
            ..Note::default()
        };
        assert!(!should_fetch_note_notebook_name(&note));
    }

    #[test]
    fn resolves_notebook_name_from_guid() {
        let note = Note {
            notebook_guid: Some("work".to_string()),
            ..Note::default()
        };
        let notebooks = vec![Notebook {
            guid: "work".to_string(),
            name: "Work".to_string(),
            stack: None,
        }];
        assert_eq!(
            note_notebook_name(&note, &notebooks).as_deref(),
            Some("Work")
        );
    }

    #[test]
    fn uses_fresh_cached_notebook_name() {
        let mut storage = Storage::memory();
        storage
            .set_notebooks(BTreeMap::from([("work".to_string(), "Work".to_string())]))
            .unwrap();
        mark_cache_updated(&mut storage, NOTEBOOKS_CACHE_UPDATED_AT).unwrap();
        let note = Note {
            notebook_guid: Some("work".to_string()),
            ..Note::default()
        };
        assert_eq!(
            cached_notebook_name(&storage, &note).as_deref(),
            Some("Work")
        );
    }

    #[test]
    fn ignores_stale_cached_notebook_name() {
        let mut storage = Storage::memory();
        storage
            .set_notebooks(BTreeMap::from([("work".to_string(), "Work".to_string())]))
            .unwrap();
        storage
            .set_setting(
                NOTEBOOKS_CACHE_UPDATED_AT,
                (current_timestamp_seconds() - METADATA_CACHE_TTL_SECONDS - 1).to_string(),
            )
            .unwrap();
        let note = Note {
            notebook_guid: Some("work".to_string()),
            ..Note::default()
        };
        assert_eq!(cached_notebook_name(&storage, &note), None);
    }

    #[test]
    fn uses_fresh_cached_tag_names() {
        let mut storage = Storage::memory();
        storage
            .set_tags(BTreeMap::from([(
                "tag-guid".to_string(),
                "project".to_string(),
            )]))
            .unwrap();
        mark_cache_updated(&mut storage, TAGS_CACHE_UPDATED_AT).unwrap();
        let note = Note {
            tag_guids: vec!["tag-guid".to_string()],
            ..Note::default()
        };
        assert_eq!(
            cached_tag_names(&storage, &note).unwrap(),
            vec!["project".to_string()]
        );
    }

    #[test]
    fn detects_notes_missing_tag_names() {
        let notes = vec![Note {
            tag_guids: vec!["tag-guid".to_string()],
            ..Note::default()
        }];
        assert!(notes_need_tag_name_lookup(&notes));
    }

    #[test]
    fn keeps_notes_with_existing_tag_names() {
        let notes = vec![Note {
            tag_guids: vec!["tag-guid".to_string()],
            tag_names: vec!["project".to_string()],
            ..Note::default()
        }];
        assert!(!notes_need_tag_name_lookup(&notes));
    }

    #[test]
    fn applies_tag_names_to_note_list() {
        let mut notes = vec![Note {
            tag_guids: vec!["tag-guid".to_string(), "other-guid".to_string()],
            ..Note::default()
        }];
        apply_tag_names_to_notes(
            &mut notes,
            &BTreeMap::from([
                ("tag-guid".to_string(), "project".to_string()),
                ("other-guid".to_string(), "work".to_string()),
            ]),
        );
        assert_eq!(
            notes[0].tag_names,
            vec!["project".to_string(), "work".to_string()]
        );
    }
}
