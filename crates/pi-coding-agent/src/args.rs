use crate::CliError;

pub const DEFAULT_MAX_TURNS: u32 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliArgs {
    pub print: bool,
    pub prompt: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: u32,
    pub help: bool,
    pub version: bool,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            print: false,
            prompt: None,
            model: None,
            api_key: None,
            system_prompt: None,
            max_turns: DEFAULT_MAX_TURNS,
            help: false,
            version: false,
        }
    }
}

pub fn help_text() -> String {
    format!(
        "pi-coding-agent {}\n\nUsage:\n  pi-coding-agent -p <prompt>\n\nOptions:\n  -p, --print              Run one prompt and print the assistant response\n  --model <id>             Model id from the built-in Rust model table\n  --api-key <key>          API key passed to the selected provider\n  --system-prompt <text>   System prompt override\n  --max-turns <n>          Maximum agent loop turns (default: 5)\n  -h, --help               Show help\n  -v, --version            Show version\n",
        env!("CARGO_PKG_VERSION")
    )
}

fn take_value(raw: &[String], index: &mut usize, flag: &str) -> Result<String, CliError> {
    let next_index = *index + 1;
    let value = raw
        .get(next_index)
        .ok_or_else(|| CliError::MissingValue(flag.to_string()))?;
    *index = next_index;
    Ok(value.clone())
}

fn parse_max_turns(value: String) -> Result<u32, CliError> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| CliError::InvalidMaxTurns(value.clone()))?;
    if parsed == 0 {
        return Err(CliError::InvalidMaxTurns(value));
    }
    Ok(parsed)
}

pub fn parse_args<I>(args: I) -> Result<CliArgs, CliError>
where
    I: IntoIterator<Item = String>,
{
    let mut parsed = CliArgs::default();
    let mut prompt_parts = Vec::new();
    let raw: Vec<String> = args.into_iter().collect();
    let mut i = 0;

    while i < raw.len() {
        let arg = &raw[i];
        match arg.as_str() {
            "-p" | "--print" => {
                parsed.print = true;
                if let Some(next) = raw.get(i + 1) {
                    if !next.starts_with('-') || next.starts_with("---") {
                        prompt_parts.push(next.clone());
                        i += 1;
                    }
                }
            }
            "-h" | "--help" => parsed.help = true,
            "-v" | "--version" => parsed.version = true,
            "--model" => parsed.model = Some(take_value(&raw, &mut i, "--model")?),
            "--api-key" => parsed.api_key = Some(take_value(&raw, &mut i, "--api-key")?),
            "--system-prompt" => {
                parsed.system_prompt = Some(take_value(&raw, &mut i, "--system-prompt")?)
            }
            "--max-turns" => {
                let value = take_value(&raw, &mut i, "--max-turns")?;
                parsed.max_turns = parse_max_turns(value)?;
            }
            value if value.starts_with("--") => {
                return Err(CliError::UnknownFlag(value.to_string()));
            }
            value if value.starts_with('-') => {
                return Err(CliError::UnknownFlag(value.to_string()));
            }
            value => prompt_parts.push(value.to_string()),
        }
        i += 1;
    }

    if !prompt_parts.is_empty() {
        parsed.prompt = Some(prompt_parts.join(" "));
    }

    Ok(parsed)
}
