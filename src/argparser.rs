use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgValue {
    Bool(bool),
    Int(i64),
    String(String),
    List(Vec<String>),
    Empty,
}

impl ArgValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn into_string(self) -> Option<String> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn into_list(self) -> Option<Vec<String>> {
        match self {
            Self::List(value) => Some(value),
            Self::String(value) => Some(vec![value]),
            _ => None,
        }
    }
}

pub type ParsedArgs = BTreeMap<String, ArgValue>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueKind {
    String,
    Int,
}

#[derive(Clone, Debug)]
pub struct OptionSpec {
    pub name: &'static str,
    pub alt_name: Option<&'static str>,
    pub help: &'static str,
    pub required: bool,
    pub repetitive: bool,
    pub value_kind: ValueKind,
    pub default: Option<ArgValue>,
    pub empty_value: Option<ArgValue>,
    pub flag_value: Option<ArgValue>,
}

impl OptionSpec {
    pub fn argument(name: &'static str, help: &'static str) -> Self {
        Self {
            name,
            alt_name: None,
            help,
            required: false,
            repetitive: false,
            value_kind: ValueKind::String,
            default: None,
            empty_value: None,
            flag_value: None,
        }
    }

    pub fn flag(name: &'static str, help: &'static str) -> Self {
        Self {
            name,
            alt_name: None,
            help,
            required: false,
            repetitive: false,
            value_kind: ValueKind::String,
            default: Some(ArgValue::Bool(false)),
            empty_value: None,
            flag_value: Some(ArgValue::Bool(true)),
        }
    }

    pub fn alt(mut self, alt_name: &'static str) -> Self {
        self.alt_name = Some(alt_name);
        self
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn repetitive(mut self) -> Self {
        self.repetitive = true;
        self
    }

    pub fn empty_value(mut self, value: ArgValue) -> Self {
        self.empty_value = Some(value);
        self
    }

    pub fn default_value(mut self, value: ArgValue) -> Self {
        self.default = Some(value);
        self
    }

    pub fn int(mut self) -> Self {
        self.value_kind = ValueKind::Int;
        self
    }
}

#[derive(Clone, Debug)]
pub struct CommandSpec {
    pub name: &'static str,
    pub help: &'static str,
    pub first_arg: Option<&'static str>,
    pub arguments: Vec<OptionSpec>,
    pub flags: Vec<OptionSpec>,
}

impl CommandSpec {
    pub fn new(name: &'static str, help: &'static str) -> Self {
        Self {
            name,
            help,
            first_arg: None,
            arguments: Vec::new(),
            flags: Vec::new(),
        }
    }

    pub fn first_arg(mut self, first_arg: &'static str) -> Self {
        self.first_arg = Some(first_arg);
        self
    }

    pub fn argument(mut self, spec: OptionSpec) -> Self {
        self.arguments.push(spec);
        self
    }

    pub fn flag(mut self, spec: OptionSpec) -> Self {
        self.flags.push(spec);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseOutcome {
    Parsed(ParsedArgs),
    Help(String),
    About,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub help: String,
}

#[derive(Clone, Debug)]
pub struct ArgParser {
    commands: Vec<CommandSpec>,
}

impl Default for ArgParser {
    fn default() -> Self {
        Self {
            commands: default_commands(),
        }
    }
}

impl ArgParser {
    pub fn new(commands: Vec<CommandSpec>) -> Self {
        Self { commands }
    }

    pub fn parse<I, S>(&self, args: I) -> Result<ParseOutcome, ParseError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let argv: Vec<String> = args.into_iter().map(Into::into).collect();
        if argv.is_empty() {
            return Ok(ParseOutcome::About);
        }

        let command_name = argv[0].as_str();
        if command_name == "autocomplete" {
            return Ok(ParseOutcome::Help(self.autocomplete(&argv[1..])));
        }

        if command_name == "--help" {
            return Ok(ParseOutcome::Help(self.help(None)));
        }

        let Some(command) = self.command(command_name) else {
            return Err(ParseError {
                message: format!("Unexpected command \"{command_name}\""),
                help: self.help(None),
            });
        };

        if argv.iter().skip(1).any(|item| item == "--help") {
            return Ok(ParseOutcome::Help(self.help(Some(command))));
        }

        let mut input: Vec<String> = argv.into_iter().skip(1).collect();
        let mut data = ParsedArgs::new();

        for spec in command.arguments.iter().chain(command.flags.iter()) {
            if let Some(default) = spec.default.clone() {
                data.insert(spec.name.to_string(), default);
            }
            if let Some(alt_name) = spec.alt_name {
                for item in &mut input {
                    if item == alt_name {
                        *item = spec.name.to_string();
                    }
                }
            }
        }

        if let Some(first_arg) = command.first_arg {
            if input.is_empty() {
                input.push(first_arg.to_string());
            } else if !self.is_known_option(command, &input[0]) {
                input.insert(0, first_arg.to_string());
            }
        }

        let mut index = 0;
        let mut active: Option<&OptionSpec> = None;
        while index < input.len() {
            let item = input[index].clone();
            if let Some(active_spec) = active.take() {
                if self.is_known_option(command, &item) {
                    if let Some(empty_value) = active_spec.empty_value.clone() {
                        insert_value(&mut data, active_spec, empty_value);
                        active = self.argument(command, &item);
                        if active.is_none() {
                            if let Some(flag) = self.flag(command, &item) {
                                data.insert(
                                    flag.name.to_string(),
                                    flag.flag_value.clone().unwrap_or(ArgValue::Bool(true)),
                                );
                            }
                        }
                        index += 1;
                        continue;
                    }
                    return Err(ParseError {
                        message: format!(
                            "Unexpected value \"{item}\" for argument \"{}\"",
                            active_spec.name
                        ),
                        help: self.help(Some(command)),
                    });
                }

                let value = convert_value(active_spec, &item).map_err(|_| ParseError {
                    message: format!(
                        "Unexpected value \"{item}\" for argument \"{}\"",
                        active_spec.name
                    ),
                    help: self.help(Some(command)),
                })?;
                insert_value(&mut data, active_spec, value);
                index += 1;
                continue;
            }

            if let Some(argument) = self.argument(command, &item) {
                active = Some(argument);
            } else if let Some(flag) = self.flag(command, &item) {
                data.insert(
                    flag.name.to_string(),
                    flag.flag_value.clone().unwrap_or(ArgValue::Bool(true)),
                );
            } else {
                return Err(ParseError {
                    message: format!(
                        "Unexpected argument \"{item}\" for command \"{}\"",
                        command.name
                    ),
                    help: self.help(Some(command)),
                });
            }
            index += 1;
        }

        if let Some(active_spec) = active {
            if let Some(empty_value) = active_spec.empty_value.clone() {
                insert_value(&mut data, active_spec, empty_value);
            } else {
                return Err(ParseError {
                    message: format!(
                        "Unexpected value \"\" for argument \"{}\"",
                        active_spec.name
                    ),
                    help: self.help(Some(command)),
                });
            }
        }

        for spec in command.arguments.iter().chain(command.flags.iter()) {
            if spec.required && !input.iter().any(|item| item == spec.name) {
                return Err(ParseError {
                    message: format!(
                        "Not found required argument \"{}\" for command \"{}\"",
                        spec.name, command.name
                    ),
                    help: self.help(Some(command)),
                });
            }
        }

        Ok(ParseOutcome::Parsed(normalize_keys(data)))
    }

    pub fn help(&self, command: Option<&CommandSpec>) -> String {
        if let Some(command) = command {
            let width = command
                .arguments
                .iter()
                .chain(command.flags.iter())
                .map(|item| item.name.len())
                .max()
                .unwrap_or(0);
            let mut output = format!("Options for: {}\nAvailable arguments:\n", command.name);
            for argument in &command.arguments {
                let default = if command.first_arg == Some(argument.name) {
                    "[default] "
                } else {
                    ""
                };
                output.push_str(&format!(
                    "{:>width$} : {default}{}\n",
                    argument.name,
                    argument.help,
                    width = width
                ));
            }
            if !command.flags.is_empty() {
                output.push_str("Available flags:\n");
                for flag in &command.flags {
                    output.push_str(&format!(
                        "{:>width$} : {}\n",
                        flag.name,
                        flag.help,
                        width = width
                    ));
                }
            }
            return output;
        }

        let width = self
            .commands
            .iter()
            .map(|item| item.name.len())
            .max()
            .unwrap_or(0);
        let mut output = "Available commands:\n".to_string();
        let mut commands = self.commands.clone();
        commands.sort_by_key(|item| item.name);
        for command in commands {
            output.push_str(&format!(
                "{:>width$} : {}\n",
                command.name,
                command.help,
                width = width
            ));
        }
        output
    }

    fn command(&self, name: &str) -> Option<&CommandSpec> {
        self.commands.iter().find(|item| item.name == name)
    }

    fn argument<'a>(&self, command: &'a CommandSpec, name: &str) -> Option<&'a OptionSpec> {
        command.arguments.iter().find(|item| item.name == name)
    }

    fn flag<'a>(&self, command: &'a CommandSpec, name: &str) -> Option<&'a OptionSpec> {
        command.flags.iter().find(|item| item.name == name)
    }

    fn is_known_option(&self, command: &CommandSpec, name: &str) -> bool {
        self.argument(command, name).is_some() || self.flag(command, name).is_some()
    }

    fn autocomplete(&self, args: &[String]) -> String {
        if args.is_empty() {
            return self
                .commands
                .iter()
                .map(|item| item.name)
                .collect::<Vec<_>>()
                .join(" ");
        }

        if let Some(command) = self.command(&args[0]) {
            return command
                .arguments
                .iter()
                .chain(command.flags.iter())
                .map(|item| item.name)
                .collect::<Vec<_>>()
                .join(" ");
        }

        self.commands
            .iter()
            .filter(|item| item.name.starts_with(&args[0]))
            .map(|item| item.name)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn convert_value(spec: &OptionSpec, value: &str) -> Result<ArgValue, ()> {
    match spec.value_kind {
        ValueKind::String => Ok(ArgValue::String(value.to_string())),
        ValueKind::Int => value.parse::<i64>().map(ArgValue::Int).map_err(|_| ()),
    }
}

fn insert_value(data: &mut ParsedArgs, spec: &OptionSpec, value: ArgValue) {
    if spec.repetitive {
        match data.remove(spec.name) {
            Some(ArgValue::List(mut values)) => {
                if let ArgValue::String(value) = value {
                    values.push(value);
                }
                data.insert(spec.name.to_string(), ArgValue::List(values));
            }
            Some(ArgValue::String(previous)) => {
                let mut values = vec![previous];
                if let ArgValue::String(value) = value {
                    values.push(value);
                }
                data.insert(spec.name.to_string(), ArgValue::List(values));
            }
            _ => {
                let values = match value {
                    ArgValue::String(value) => vec![value],
                    _ => Vec::new(),
                };
                data.insert(spec.name.to_string(), ArgValue::List(values));
            }
        }
    } else {
        data.insert(spec.name.to_string(), value);
    }
}

fn normalize_keys(data: ParsedArgs) -> ParsedArgs {
    data.into_iter()
        .map(|(key, value)| (key.trim_start_matches('-').replace('-', "_"), value))
        .collect()
}

pub fn default_commands() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("user", "Show information about active user.")
            .flag(OptionSpec::flag("--full", "Show full information.")),
        CommandSpec::new("login", "Authorize in Evernote."),
        CommandSpec::new("logout", "Logout from Evernote.")
            .flag(OptionSpec::flag("--force", "Don't ask about logging out.")),
        CommandSpec::new("settings", "Show and edit current settings.")
            .argument(
                OptionSpec::argument("--editor", "Set the editor.")
                    .empty_value(ArgValue::String("#GET#".to_string())),
            )
            .argument(
                OptionSpec::argument("--note_ext", "Set note extensions.")
                    .empty_value(ArgValue::String("#GET#".to_string())),
            )
            .argument(
                OptionSpec::argument("--extras", "Set markdown extras.")
                    .empty_value(ArgValue::String("#GET#".to_string())),
            ),
        CommandSpec::new("create", "Create note in Evernote.")
            .argument(
                OptionSpec::argument("--title", "The note title.")
                    .alt("-t")
                    .required(),
            )
            .argument(
                OptionSpec::argument("--content", "The note content.")
                    .alt("-c")
                    .default_value(ArgValue::String("WRITE".to_string())),
            )
            .argument(
                OptionSpec::argument("--tag", "Tag to be added to the note.")
                    .alt("-tg")
                    .repetitive(),
            )
            .argument(OptionSpec::argument("--created", "Set local creation time.").alt("-cr"))
            .argument(
                OptionSpec::argument("--resource", "Add a resource to the note.")
                    .alt("-rs")
                    .repetitive(),
            )
            .argument(OptionSpec::argument("--notebook", "Set the notebook.").alt("-nb"))
            .argument(OptionSpec::argument("--reminder", "Set reminder date/time.").alt("-r"))
            .argument(OptionSpec::argument("--url", "Set the URL for the note.").alt("-u"))
            .flag(OptionSpec::flag("--raw", "Edit note with raw ENML.").alt("-r"))
            .flag(OptionSpec::flag("--rawmd", "Edit note with raw markdown.").alt("-rm")),
        CommandSpec::new("create-linked", "Create linked note in Evernote.")
            .first_arg("--notebook")
            .argument(
                OptionSpec::argument("--title", "The note title.")
                    .alt("-t")
                    .required(),
            )
            .argument(
                OptionSpec::argument("--notebook", "Linked notebook name.")
                    .alt("-nb")
                    .required(),
            ),
        CommandSpec::new("find", "Search notes in Evernote.")
            .first_arg("--search")
            .argument(
                OptionSpec::argument("--search", "Text to search.")
                    .alt("-s")
                    .empty_value(ArgValue::String("*".to_string())),
            )
            .argument(
                OptionSpec::argument("--tag", "Tag sought on the notes.")
                    .alt("-tg")
                    .repetitive(),
            )
            .argument(
                OptionSpec::argument("--notebook", "Notebook containing the notes.").alt("-nb"),
            )
            .argument(OptionSpec::argument("--date", "Date or date range.").alt("-d"))
            .argument(
                OptionSpec::argument("--count", "How many notes to show.")
                    .alt("-cn")
                    .int(),
            )
            .flag(OptionSpec::flag("--content", "Search by content.").alt("-cs"))
            .flag(OptionSpec::flag("--exact-entry", "Search exact entry.").alt("-ee"))
            .flag(OptionSpec::flag("--guid", "Show GUIDs.").alt("-id"))
            .flag(OptionSpec::flag("--ignore-completed", "Only unfinished reminders.").alt("-C"))
            .flag(OptionSpec::flag("--reminders-only", "Only notes with reminders.").alt("-R"))
            .flag(OptionSpec::flag("--deleted-only", "Only deleted notes.").alt("-D"))
            .flag(OptionSpec::flag("--with-notebook", "Show notebook names.").alt("-wn"))
            .flag(OptionSpec::flag("--with-tags", "Show tags.").alt("-wt"))
            .flag(OptionSpec::flag("--with-url", "Show web URLs.").alt("-wu")),
        CommandSpec::new("edit", "Edit note in Evernote.")
            .first_arg("--note")
            .argument(
                OptionSpec::argument("--note", "Note name, GUID, or previous search ID.")
                    .alt("-n")
                    .required(),
            )
            .argument(OptionSpec::argument("--title", "Set new title.").alt("-t"))
            .argument(OptionSpec::argument("--content", "Set new content.").alt("-c"))
            .argument(
                OptionSpec::argument("--resource", "Add a resource.")
                    .alt("-rs")
                    .repetitive(),
            )
            .argument(
                OptionSpec::argument("--tag", "Set tags.")
                    .alt("-tg")
                    .repetitive(),
            )
            .argument(OptionSpec::argument("--created", "Set local creation time.").alt("-cr"))
            .argument(OptionSpec::argument("--notebook", "Assign notebook.").alt("-nb"))
            .argument(OptionSpec::argument("--reminder", "Set reminder.").alt("-r"))
            .argument(OptionSpec::argument("--url", "Set URL.").alt("-u"))
            .flag(OptionSpec::flag("--raw", "Edit note with raw ENML.").alt("-r"))
            .flag(OptionSpec::flag("--rawmd", "Edit note with raw markdown.").alt("-rm")),
        CommandSpec::new("edit-linked", "Edit linked note.")
            .first_arg("--notebook")
            .argument(
                OptionSpec::argument("--notebook", "Linked notebook name.")
                    .alt("-nb")
                    .required(),
            )
            .argument(
                OptionSpec::argument("--note", "Note title.")
                    .alt("-n")
                    .required(),
            ),
        CommandSpec::new("show", "Output note in the terminal.")
            .first_arg("--note")
            .argument(
                OptionSpec::argument("--note", "Note name, GUID, or previous search ID.")
                    .alt("-n")
                    .required(),
            )
            .flag(OptionSpec::flag("--raw", "Show raw note body.").alt("-w")),
        CommandSpec::new("remove", "Remove note from Evernote.")
            .first_arg("--note")
            .argument(
                OptionSpec::argument("--note", "Note name, GUID, or previous search ID.")
                    .alt("-n")
                    .required(),
            )
            .flag(OptionSpec::flag("--force", "Don't ask about removing.").alt("-f")),
        CommandSpec::new("dedup", "Preview duplicate notes without removing them.")
            .argument(OptionSpec::argument("--notebook", "Notebook to search.").alt("-nb")),
        CommandSpec::new("notebook-list", "Show notebooks.")
            .flag(OptionSpec::flag("--guid", "Show notebook GUIDs.").alt("-id")),
        CommandSpec::new("notebook-create", "Create notebook.")
            .argument(
                OptionSpec::argument("--title", "New notebook title.")
                    .alt("-t")
                    .required(),
            )
            .argument(OptionSpec::argument("--stack", "Notebook stack.")),
        CommandSpec::new("notebook-edit", "Rename notebook.")
            .first_arg("--notebook")
            .argument(
                OptionSpec::argument("--notebook", "Notebook name.")
                    .alt("-nb")
                    .required(),
            )
            .argument(OptionSpec::argument("--title", "New notebook title.").alt("-t")),
        CommandSpec::new("notebook-remove", "Remove notebook.")
            .first_arg("--notebook")
            .argument(
                OptionSpec::argument("--notebook", "Notebook name.")
                    .alt("-nb")
                    .required(),
            )
            .flag(OptionSpec::flag(
                "--force",
                "Don't ask about removing notebook.",
            )),
        CommandSpec::new("tag-list", "Show tags.")
            .flag(OptionSpec::flag("--guid", "Show tag GUIDs.").alt("-id")),
        CommandSpec::new("tag-create", "Create tag.").argument(
            OptionSpec::argument("--title", "New tag title.")
                .alt("-t")
                .required(),
        ),
        CommandSpec::new("tag-edit", "Rename tag.")
            .first_arg("--tagname")
            .argument(
                OptionSpec::argument("--tagname", "Tag name.")
                    .alt("-tgn")
                    .required(),
            )
            .argument(OptionSpec::argument("--title", "New tag title.").alt("-t")),
        CommandSpec::new("tag-remove", "Remove tag.")
            .first_arg("--tagname")
            .argument(
                OptionSpec::argument("--tagname", "Tag name.")
                    .alt("-tgn")
                    .required(),
            )
            .flag(OptionSpec::flag("--force", "Don't ask about removing.")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn testing_parser() -> ArgParser {
        ArgParser::new(vec![
            CommandSpec::new("testing", "Create note")
                .first_arg("--test_req_arg")
                .argument(
                    OptionSpec::argument("--test_req_arg", "Set note title")
                        .alt("-tra")
                        .required(),
                )
                .argument(
                    OptionSpec::argument("--test_arg", "Add tag to note")
                        .alt("-ta")
                        .empty_value(ArgValue::Empty),
                )
                .argument(OptionSpec::argument("--test_arg2", "Add tag to note").alt("-ta2"))
                .flag(OptionSpec::flag("--test_flag", "Add tag to note").alt("-tf")),
        ])
    }

    fn parsed(args: &[&str]) -> ParsedArgs {
        match testing_parser().parse(args.iter().copied()) {
            Ok(ParseOutcome::Parsed(values)) => values,
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn empty_command_prints_about() {
        assert_eq!(
            testing_parser().parse(Vec::<String>::new()).unwrap(),
            ParseOutcome::About
        );
    }

    #[test]
    fn rejects_unknown_command() {
        assert!(testing_parser().parse(["testing_err"]).is_err());
    }

    #[test]
    fn parses_full_arguments() {
        let mut expected = ParsedArgs::new();
        expected.insert(
            "test_req_arg".to_string(),
            ArgValue::String("test_req_val".to_string()),
        );
        expected.insert("test_flag".to_string(), ArgValue::Bool(true));
        expected.insert(
            "test_arg".to_string(),
            ArgValue::String("test_val".to_string()),
        );
        assert_eq!(
            parsed(&[
                "testing",
                "--test_req_arg",
                "test_req_val",
                "--test_flag",
                "--test_arg",
                "test_val"
            ]),
            expected
        );
    }

    #[test]
    fn inserts_default_first_arg() {
        let mut expected = ParsedArgs::new();
        expected.insert(
            "test_req_arg".to_string(),
            ArgValue::String("test_def_val".to_string()),
        );
        expected.insert("test_flag".to_string(), ArgValue::Bool(false));
        assert_eq!(parsed(&["testing", "test_def_val"]), expected);
    }

    #[test]
    fn supports_empty_values_and_short_names() {
        let mut expected = ParsedArgs::new();
        expected.insert(
            "test_req_arg".to_string(),
            ArgValue::String("test_def_val".to_string()),
        );
        expected.insert("test_arg".to_string(), ArgValue::Empty);
        expected.insert(
            "test_arg2".to_string(),
            ArgValue::String("test_arg2_val".to_string()),
        );
        expected.insert("test_flag".to_string(), ArgValue::Bool(false));
        assert_eq!(
            parsed(&["testing", "test_def_val", "-ta", "-ta2", "test_arg2_val"]),
            expected
        );
    }

    #[test]
    fn parses_builtin_find_count_as_integer() {
        let parser = ArgParser::default();
        let outcome = parser.parse(["find", "needle", "--count", "3"]).unwrap();
        let ParseOutcome::Parsed(values) = outcome else {
            panic!("expected parsed values");
        };
        assert_eq!(
            values.get("search"),
            Some(&ArgValue::String("needle".to_string()))
        );
        assert_eq!(values.get("count"), Some(&ArgValue::Int(3)));
    }

    #[test]
    fn parses_builtin_find_content_flag() {
        let parser = ArgParser::default();
        let outcome = parser.parse(["find", "needle", "--content"]).unwrap();
        let ParseOutcome::Parsed(values) = outcome else {
            panic!("expected parsed values");
        };
        assert_eq!(values.get("content"), Some(&ArgValue::Bool(true)));
    }
}
