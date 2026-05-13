Rust rewrite of https://github.com/vitaly-zdanevich/geeknote

Reeknote for Evernote
===

Reeknote is a command-line client for Evernote. It is intended for Linux,
FreeBSD, macOS, and other systems where a Rust binary can run.

It allows you to:

* create notes in your Evernote account;
* create tags and notebooks;
* search Evernote from the console using filters;
* show notes in the terminal;
* edit notes using any editor, such as nano, vim, emacs, or mcedit;
* download notes to local files with `gnsync`;
* use Evernote from cron jobs and scripts.

This document shows how to work with Evernote notes, notebooks, tags, and
`gnsync` using Reeknote.

## Installation

### Build From Source

Install the stable Rust toolchain, then build both binaries:

```sh
cargo build --release --bins
```

The binaries will be written to:

* `target/release/reeknote`
* `target/release/gnsync`

To build only one binary:

```sh
cargo build --release --bin reeknote
cargo build --release --bin gnsync
```

### Install Locally With Cargo

From the repository root:

```sh
cargo install --path .
```

This installs the `reeknote` and `gnsync` binaries into Cargo's binary
directory, usually `~/.cargo/bin`.

### Uninstallation

If installed with `cargo install --path .`:

```sh
cargo uninstall reeknote
```

## Testing

Reeknote has a non-destructive unit test suite.

Run the tests with:

```sh
cargo test
```

Run formatting and lint checks with:

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
```

## CI/CD

The GitLab CI pipeline in `.gitlab-ci.yml` runs:

* formatting checks;
* Clippy lints;
* tests;
* Linux x86_64 release builds;
* Linux ARM64 release builds.

Each build uploads an artifact containing `reeknote` and `gnsync`.

The runner tags in `.gitlab-ci.yml` target GitLab.com hosted Linux runners. If
this project uses self-managed or differently tagged runners, adjust the
`tags` values.

## Reeknote Settings

### Authorizing Reeknote

After installation, Reeknote must be authorized with Evernote before use. To
authorize, run:

```sh
reeknote login
```

This starts the authorization process. Reeknote asks for a developer token or
uses OAuth, then stores the token in the local database. Re-authorization is not
required unless you log out or change users.

After authorization, you can start to work with Evernote.

### Logging Out And Changing Users

To change Evernote users, run:

```sh
reeknote logout
```

Afterward, repeat the authorization step.

### Yinxiang Biji Notes

To use Evernote's separate service in China, Yinxiang Biji, set
`REEKNOTE_BASE` to `yinxiang`.

```sh
REEKNOTE_BASE=yinxiang reeknote login

# Or:
export REEKNOTE_BASE=yinxiang
reeknote ...commands...
```

Yinxiang Biji can be faster in China and supports Chinese payment methods. Be
aware that its data is stored on servers in China.

### Login With A Developer Token

Reeknote can use an Evernote developer token:

```sh
EVERNOTE_DEV_TOKEN=... reeknote login
```

You can request Evernote API access from Evernote Developer Support. When asked
for the application name, use `reeknote`.

### Examining Your Settings

```sh
$ reeknote settings
Reeknote
******************************
Version: 3.0.24
App dir: /home/username/.reeknote
Error log: /home/username/.reeknote/error.log
Editor: nano
Markdown2 Extras: None
Note extension: [".markdown", ".org"]
******************************
Username: username
Id: 11111111
Email: example@example.com
```

### Setting Up The Default Editor

You can edit notes in console editors in plain text or Markdown format.

Check the current editor:

```sh
reeknote settings --editor
```

Change the default editor:

```sh
reeknote settings --editor vim
```

To use `gvim`, prevent forking from the terminal with `-f`:

```sh
reeknote settings --editor 'gvim -f'
```

Example:

```sh
$ reeknote settings --editor
Current editor is: nano
$ reeknote settings --editor vim
Changes saved.
$ reeknote settings --editor
Current editor is: vim
```

### Enabling Markdown Extras

Check the currently enabled Markdown extras:

```sh
reeknote settings --extras
```

Change them:

```sh
reeknote settings --extras "tables, footnotes"
```

Example:

```sh
$ reeknote settings --extras
Current markdown2 extras is : None
$ reeknote settings --extras "tables, footnotes"
Changes saved.
```

## Working With Notes

### Notes: Creating Notes

The main functionality is creating notes in Evernote.

Synopsis:

```sh
reeknote create --title <title>
                [--content <content>]
                [--tag <tag>]
                [--created <date and time>]
                [--resource <attachment filename>]
                [--notebook <notebook where to save>]
                [--reminder <date and time>]
                [--url <url>]
                [--raw]
                [--rawmd]
```

Options:

| Option | Argument | Description |
|--------|----------|-------------|
| `--title` | title | Title of the new note. |
| `--content` | content | Content of the new note. If omitted, Reeknote opens your editor. |
| `--tag` | tag | Tag for the note. May be repeated. |
| `--created` | date | Creation date in `yyyy-mm-dd` or `yyyy-mm-dd HH:MM` format. |
| `--resource` | filename | File to attach to the note. May be repeated. |
| `--notebook` | notebook | Notebook where the note should be saved. |
| `--reminder` | date | Reminder date/time. Also supports `TOMORROW`, `WEEK`, `NONE`, and `DONE`. |
| `--url` | url | Source URL for the note. |
| `--raw` | | Treat content as raw ENML. |
| `--rawmd` | | Treat content as raw Markdown. |

Example:

```sh
reeknote create --title "Shopping list" \
                --content "Don't forget to buy milk, turkey and chips." \
                --resource shoppinglist.pdf \
                --notebook "Family" \
                --tag "shop" --tag "holiday" --tag "important"
```

Create a note and edit the content in your editor:

```sh
reeknote create --title "Meeting with customer" \
                --notebook "Meetings" \
                --tag "projectA" --tag "important" --tag "report" \
                --created "2015-10-23 14:30"
```

### Notes: Searching For Notes

Search notes in Evernote and output results in the terminal.

Synopsis:

```sh
reeknote find --search <text to find>
              [--tag <tag>]
              [--notebook <notebook>]
              [--date <date or date range>]
              [--count <how many results to show>]
              [--exact-entry]
              [--content]
              [--reminders-only]
              [--deleted-only]
              [--ignore-completed]
              [--with-tags]
              [--with-notebook]
              [--with-url]
              [--guid]
```

`find` searches Evernote and remembers the last result. You can use the numeric
result ID for future commands.

Example:

```sh
$ reeknote find --search "Shopping"
Found 2 items
  1 : 2006-06-02 2009-01-19 Grocery Shopping List
  2 : 2015-02-22 2015-02-24 Gift Shopping List

$ reeknote show 2
```

Options:

| Option | Argument | Description |
|--------|----------|-------------|
| `--search` | text to find | Text to find. Use `*` for broad searches. |
| `--tag` | tag | Filter by tag. May be repeated. |
| `--notebook` | notebook | Filter by notebook. |
| `--date` | date or range | Filter by date, such as `yyyy-mm-dd` or `yyyy-mm-dd/yyyy-mm-dd`. |
| `--count` | count | Limit the number of displayed results. |
| `--content` | | Search by note content instead of title. |
| `--exact-entry` | | Search exact entries instead of fuzzy entries. |
| `--guid` | | Show GUID instead of numeric result index. |
| `--ignore-completed` | | Include only unfinished reminders. |
| `--reminders-only` | | Include only notes with reminders. |
| `--deleted-only` | | Include only deleted/trashed notes. |
| `--with-notebook` | | Show notebook name. |
| `--with-tags` | | Show tags. |
| `--with-url` | | Show Evernote web-client URL for each note. |

Examples:

```sh
reeknote find --search "How to patch KDE2" --notebook "jokes" --date 2015-10-14/2015-10-28
reeknote find --search "apt-get install apache nginx" --content --notebook "manual"
```

### Notes: Editing Notes

Edit notes in Evernote using any editor you like.

Synopsis:

```sh
reeknote edit --note <title or GUID of note to edit>
              [--title <the new title>]
              [--content <new content or "WRITE">]
              [--resource <attachment filename>]
              [--tag <tag>]
              [--created <date and time>]
              [--notebook <new notebook>]
              [--reminder <date and time>]
              [--url <url>]
              [--raw]
              [--rawmd]
```

Options:

| Option | Argument | Description |
|--------|----------|-------------|
| `--note` | title, GUID, or search result ID | Note to edit. If multiple notes match, Reeknote asks you to choose. |
| `--title` | new title | Rename the note. |
| `--content` | content or `WRITE` | Replace note content, or open the current content in an editor. |
| `--resource` | filename | Attach a file. May be repeated. Replaces existing resources. |
| `--tag` | tag | Set tags. May be repeated. Replaces existing tags. |
| `--created` | date | Set creation date in `yyyy-mm-dd` or `yyyy-mm-dd HH:MM` format. |
| `--notebook` | notebook | Move the note to another notebook. |
| `--reminder` | date | Set reminder date/time. Also supports `TOMORROW`, `WEEK`, `NONE`, `DONE`, and `DELETE`. |
| `--url` | url | Set the source URL. |
| `--raw` | | Treat content as raw ENML. |
| `--rawmd` | | Treat content as raw Markdown. |

Examples:

```sh
reeknote edit --note "Naughty List" --title "Nice List"
reeknote edit --note "Naughty List" --title "Nice List" --content "WRITE"
```

### Notes: Showing Note Content

Output any note in the terminal with `show`. Reeknote can show a note by title,
GUID, or previous search result ID. If a search finds multiple notes, Reeknote
asks you to choose.

When output goes to an interactive terminal, Reeknote visually highlights
Evernote code blocks, inline code, quote blocks, italic text, and bold text.
Inline links are rendered as blue Markdown links. Redirected output stays plain
Markdown text. In Kitty, image attachments are shown inline in the terminal.
Other terminals and redirected output show image placeholders with their file
names, such as `[Image: photo.png]`.

If a note has audio attachments and `show` is running in an interactive
terminal, Reeknote asks whether to play them with the local `mpv` player. When
you confirm, Reeknote downloads the audio attachment data to temporary files,
opens them in `mpv`, and removes the temporary files after playback exits.

Synopsis:

```sh
reeknote show <text, GUID, or previous search result ID>
```

Examples:

```sh
$ reeknote show "Shop*"

Found 2 items
  1 : Grocery Shopping List
  2 : Gift Shopping List
  0 : -Cancel-
: _
```

Use a previous search result:

```sh
$ reeknote find --search "Shop*"
Found 2 items
  1 : Grocery Shopping List
  2 : Gift Shopping List

$ reeknote show 2
################### URL ###################
NoteLink: evernote:///view/111/s1/note-guid/note-guid/
WebClientURL: https://www.evernote.com/shard/s1/nl/111/note-guid
################## TITLE ##################
Gift Shopping List
=================== META ==================
Notebook: Personal
Tags: shopping, gifts
Created: 2015-02-22
Updated: 2015-02-24
||||||||||||||| REMINDERS |||||||||||||||||
Order: None
Time: None
Done: None
---------------- CONTENT -----------------
Coffee
Chocolate
```

Raw ENML output:

```sh
reeknote show 2 --raw
```

### Notes: Removing Notes

Remove notes from Evernote.

Synopsis:

```sh
reeknote remove --note <note name or GUID>
                [--force]
```

Options:

| Option | Argument | Description |
|--------|----------|-------------|
| `--note` | note name, GUID, or search result ID | Note to delete. If multiple notes match, Reeknote asks you to choose. |
| `--force` | | Do not ask for confirmation. |

Example:

```sh
reeknote remove --note "Shopping list"
```

### Notes: De-duplicating Notes

Reeknote can preview duplicate notes.

Synopsis:

```sh
reeknote dedup [--notebook <notebook>]
```

Options:

| Option | Argument | Description |
|--------|----------|-------------|
| `--notebook` | notebook | Filter by notebook. |

This command currently previews duplicates and does not delete notes.

Example:

```sh
reeknote dedup --notebook Contacts
```

## Working With Linked Notebooks

### Linked Notes: Creating A Note

```sh
reeknote create-linked --notebook <linked notebook> --title <title>
```

### Linked Notes: Editing A Note

```sh
reeknote edit-linked --notebook <linked notebook> --note <note title>
```

## Working With Notebooks

### Notebooks: Showing The List Of Notebooks

```sh
reeknote notebook-list [--guid]
```

Options:

| Option | Argument | Description |
|--------|----------|-------------|
| `--guid` | | Show GUID instead of numeric result index. |

### Notebooks: Creating A Notebook

```sh
reeknote notebook-create --title <notebook title>
```

Options:

| Option | Argument | Description |
|--------|----------|-------------|
| `--title` | notebook title | Title of the new notebook. |
| `--stack` | stack | Optional notebook stack. |

Example:

```sh
reeknote notebook-create --title "Sport diets"
```

### Notebooks: Renaming A Notebook

```sh
reeknote notebook-edit --notebook <old name> --title <new name>
```

Options:

| Option | Argument | Description |
|--------|----------|-------------|
| `--notebook` | old name | Existing notebook to rename. |
| `--title` | new name | New notebook title. |

Example:

```sh
reeknote notebook-edit --notebook "Sport diets" --title "Hangover"
```

### Notebooks: Removing A Notebook

```sh
reeknote notebook-remove --notebook <notebook> [--force]
```

Options:

| Option | Argument | Description |
|--------|----------|-------------|
| `--notebook` | notebook | Notebook to delete. |
| `--force` | | Do not ask for confirmation. |

Example:

```sh
reeknote notebook-remove --notebook "Sport diets" --force
```

## Working With Tags

### Tags: Showing The List Of Tags

```sh
reeknote tag-list [--guid]
```

Options:

| Option | Argument | Description |
|--------|----------|-------------|
| `--guid` | | Show GUID instead of numeric result index. |

### Tags: Creating A New Tag

```sh
reeknote tag-create --title <tag name to create>
```

Options:

| Option | Argument | Description |
|--------|----------|-------------|
| `--title` | tag name | Name of the tag to create. |

Example:

```sh
reeknote tag-create --title "Hobby"
```

### Tags: Renaming A Tag

```sh
reeknote tag-edit --tagname <old name> --title <new name>
```

Options:

| Option | Argument | Description |
|--------|----------|-------------|
| `--tagname` | old name | Existing tag to rename. |
| `--title` | new name | New tag name. |

Example:

```sh
reeknote tag-edit --tagname "Hobby" --title "Girls"
```

### Tags: Removing A Tag

```sh
reeknote tag-remove --tagname <tag name> [--force]
```

Options:

| Option | Argument | Description |
|--------|----------|-------------|
| `--tagname` | tag name | Existing tag to remove. |
| `--force` | | Do not ask for confirmation. |

## gnsync - Synchronization App

`gnsync` is an additional application built with Reeknote. In this Rust client,
`gnsync` currently downloads notes to local files. It does not create, update,
or delete Evernote notes.

Synopsis:

```sh
gnsync --path <path to directory>
       [--mask <unix shell-style wildcard, such as *.md>]
       [--format <plain|markdown|html>]
       [--notebook <notebook>]
       [--all]
       [--all-linked]
       [--count <count>]
       [--download-only]
       [--save-images]
       [--images-in-subdir]
```

Options:

| Option | Argument | Description |
|--------|----------|-------------|
| `--path` | directory | Directory where notes will be downloaded. |
| `--mask` | wildcard | File mask used by local sync helpers. |
| `--format` | `plain`, `markdown`, or `html` | Output format. |
| `--notebook` | notebook | Notebook to download. |
| `--all` | | Download all regular notebooks into subdirectories. |
| `--all-linked` | | Download all linked notebooks into subdirectories. |
| `--count` | count | Maximum notes to download per notebook/search. |
| `--download-only` | | Accepted for compatibility; the Rust implementation is download-only. |
| `--save-images` | | Save image resources referenced by notes. |
| `--images-in-subdir` | | Save images into a per-note image subdirectory. |

Examples:

```sh
gnsync --path ~/notes --notebook "Work" --format markdown --download-only
gnsync --path ~/evernote-backup --all --format html --save-images --images-in-subdir
```

## Original Contributors

* Vitaliy Rodnenko
* Simon Moiseenko
* Ivan Gureev
* Roman Gladkov
* Greg V
* Ilya Shmygol

## Evernote Related Projects Worth Mentioning

* [NixNote: GUI, storing notes in SQLite, written in C++](https://github.com/robert7/nixnote2)
* [CLInote: CLI, written in Go](https://github.com/TcM1911/clinote)
