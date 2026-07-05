use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process;

const DEFAULT_OPENCODE_CONFIG: &str = "~/.config/opencode/opencode.json";
const DEFAULT_CLAUDE_CONFIG: &str = "~/.claude/settings.json";
const DEFAULT_ZED_CONFIG: &str = "~/.config/zed/settings.json";

fn strip_jsonc_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_string = false;
    let mut esc = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '/' if matches!(chars.peek(), Some('/')) => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if matches!(chars.peek(), Some('*')) => {
                chars.next();
                let mut prev = '\0';
                for c in chars.by_ref() {
                    if prev == '*' && c == '/' {
                        break;
                    }
                    prev = c;
                }
            }
            _ => out.push(c),
        }
    }
    out
}

fn normalize_jsonc(s: &str) -> String {
    let no_comments = strip_jsonc_comments(s);
    let mut out = String::with_capacity(no_comments.len());
    let mut in_string = false;
    let mut esc = false;
    let chars: Vec<char> = no_comments.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            ',' => {
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_whitespace() {
                    j += 1;
                }
                if j >= chars.len() || chars[j] == ']' || chars[j] == '}' {
                    i += 1;
                    continue;
                }
                out.push(c);
            }
            _ => out.push(c),
        }
        i += 1;
    }
    out
}

fn get_api_key(arg_key: Option<String>) -> String {
    if let Some(k) = arg_key {
        if !k.is_empty() {
            return k;
        }
    }
    if let Ok(key) = env::var("KAIRUX_API_KEY") {
        return key;
    }
    match rpassword::prompt_password("Kairux API Key: ") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            eprintln!("Error: No API key provided");
            process::exit(1);
        }
    }
}

fn fetch_config(api_key: &str, baseurl: &str) -> Value {
    let api_url = format!("{}/v1/internal/config/opencode", baseurl);
    match ureq::get(&api_url)
        .set("Authorization", &format!("Bearer {}", api_key))
        .set("User-Agent", "Mozilla/5.0")
        .call()
    {
        Ok(resp) => resp.into_json().unwrap_or_else(|e| {
            eprintln!("JSON parse error: {}", e);
            process::exit(1);
        }),
        Err(e) => {
            eprintln!("Request failed: {}", e);
            process::exit(1);
        }
    }
}

fn prompt_model(models: &[String], role: &str) -> String {
    println!("\nAvailable models:");
    for (i, m) in models.iter().enumerate() {
        println!("  {}. {}", i + 1, m);
    }
    loop {
        print!("{} model (1-{}): ", role, models.len());
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        if let Ok(n) = input.trim().parse::<usize>() {
            if (1..=models.len()).contains(&n) {
                return models[n - 1].clone();
            }
        }
        println!("Invalid input");
    }
}

fn sync_claude(
    remote_models: &serde_json::Map<String, Value>,
    claude_path: &PathBuf,
    apply: bool,
    api_key: &str,
    baseurl: &str,
) {
    if !claude_path.exists() {
        eprintln!("Claude config not found: {}", claude_path.display());
        return;
    }

    println!("\nReading Claude config: {}", claude_path.display());
    let cfg_str = match fs::read_to_string(claude_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Read error: {}", e);
            return;
        }
    };
    let mut cfg: Value = match serde_json::from_str(&cfg_str) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("JSON parse error: {}", e);
            return;
        }
    };

    let models: Vec<String> = remote_models.keys().cloned().collect();
    if models.is_empty() {
        eprintln!("No models available");
        return;
    }

    let env = cfg.get("env").cloned().unwrap_or(json!({}));
    let old_opus = env
        .get("ANTHROPIC_DEFAULT_OPUS_MODEL")
        .and_then(|v| v.as_str())
        .unwrap_or("(not set)");
    let old_sonnet = env
        .get("ANTHROPIC_DEFAULT_SONNET_MODEL")
        .and_then(|v| v.as_str())
        .unwrap_or("(not set)");
    let old_haiku = env
        .get("ANTHROPIC_DEFAULT_HAIKU_MODEL")
        .and_then(|v| v.as_str())
        .unwrap_or("(not set)");

    println!("\nCurrent Claude config mapping:");
    println!("  Opus:   {}", old_opus);
    println!("  Sonnet: {}", old_sonnet);
    println!("  Haiku:  {}", old_haiku);

    let opus = prompt_model(&models, "Opus");
    let sonnet = prompt_model(&models, "Sonnet");
    let haiku = prompt_model(&models, "Haiku");

    println!("\nClaude config mapping changes:");
    println!("  Opus:   {} -> {}", old_opus, opus);
    println!("  Sonnet: {} -> {}", old_sonnet, sonnet);
    println!("  Haiku:  {} -> {}", old_haiku, haiku);

    if !apply {
        println!("\n[Dry-run] Use --apply to write changes");
        return;
    }

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let bak_path = claude_path.with_file_name(format!("settings.json.bak.{}", timestamp));
    fs::copy(claude_path, &bak_path).unwrap_or_else(|e| {
        eprintln!("Backup failed: {}", e);
        process::exit(1);
    });
    println!("\nBackup saved: {}", bak_path.display());

    let mut env = cfg.get("env").cloned().unwrap_or(json!({}));
    env["ANTHROPIC_BASE_URL"] = json!(baseurl);
    env["ANTHROPIC_AUTH_TOKEN"] = json!(api_key);
    env["ANTHROPIC_DEFAULT_OPUS_MODEL"] = json!(opus);
    env["ANTHROPIC_DEFAULT_SONNET_MODEL"] = json!(sonnet);
    env["ANTHROPIC_DEFAULT_HAIKU_MODEL"] = json!(haiku);
    cfg["env"] = env;

    fs::write(
        claude_path,
        serde_json::to_string_pretty(&cfg).unwrap_or_else(|e| {
            eprintln!("JSON write error: {}", e);
            process::exit(1);
        }) + "\n",
    )
    .unwrap_or_else(|e| {
        eprintln!("Write error: {}", e);
        process::exit(1);
    });

    println!("Claude config updated: {}", claude_path.display());
}

fn sync_zed(remote_models: &serde_json::Map<String, Value>, zed_path: &PathBuf, apply: bool) {
    if !zed_path.exists() {
        eprintln!("Zed config not found: {}", zed_path.display());
        return;
    }

    println!("\nReading Zed config: {}", zed_path.display());
    let cfg_str = match fs::read_to_string(zed_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Read error: {}", e);
            return;
        }
    };
    let cleaned = normalize_jsonc(&cfg_str);
    let mut cfg: Value = match serde_json::from_str(&cleaned) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("JSON parse error: {}", e);
            return;
        }
    };

    let new_models: Vec<Value> = remote_models
        .iter()
        .map(|(id, m)| {
            let limit = m.get("limit").cloned().unwrap_or(json!({}));
            let context = limit.get("context").and_then(|v| v.as_u64()).unwrap_or(0);
            let output = limit.get("output").and_then(|v| v.as_u64()).unwrap_or(0);
            let modalities = m.get("modalities").cloned().unwrap_or(json!({}));
            let input_mods = modalities
                .get("input")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let images = input_mods.iter().any(|s| s == "image");
            let reasoning = m.get("reasoning").and_then(|v| v.as_bool()).unwrap_or(false);
            let display_name = id.to_string();
            json!({
                "name": display_name,
                "max_tokens": context,
                "max_output_tokens": output,
                "max_completion_tokens": context,
                "capabilities": {
                    "tools": true,
                    "images": images,
                    "parallel_tool_calls": false,
                    "prompt_cache_key": false,
                    "chat_completions": true,
                    "interleaved_reasoning": reasoning
                }
            })
        })
        .collect();

    let old_arr = cfg
        .get("language_models")
        .and_then(|v| v.get("openai_compatible"))
        .and_then(|v| v.get("kairux"))
        .and_then(|v| v.get("available_models"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let old_names: std::collections::HashSet<String> = old_arr
        .iter()
        .filter_map(|m| m.get("name").and_then(|v| v.as_str().map(String::from)))
        .collect();
    let new_names: std::collections::HashSet<String> = new_models
        .iter()
        .filter_map(|m| m.get("name").and_then(|v| v.as_str().map(String::from)))
        .collect();

    let to_add: Vec<_> = new_names.difference(&old_names).collect();
    let to_remove: Vec<_> = old_names.difference(&new_names).collect();
    let changed = {
        let old_by_name: std::collections::HashMap<String, Value> = old_arr
            .iter()
            .filter_map(|m| {
                m.get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| (s.to_string(), m.clone()))
            })
            .collect();
        let new_by_name: std::collections::HashMap<String, Value> = new_models
            .iter()
            .filter_map(|m| {
                m.get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| (s.to_string(), m.clone()))
            })
            .collect();
        old_names
            .intersection(&new_names)
            .filter(|n| old_by_name.get(*n) != new_by_name.get(*n))
            .count()
    };

    println!("\nZed sync summary:");
    println!("  Total in config:   {}", old_arr.len());
    println!("  Total from API:    {}", new_models.len());
    println!("  To add:            {}", to_add.len());
    println!("  To remove:         {}", to_remove.len());
    println!("  Updated:           {}", changed);
    if !to_add.is_empty() {
        let mut sorted: Vec<_> = to_add.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        println!("  New models:        {}", sorted.join(", "));
    }
    if !to_remove.is_empty() {
        let mut sorted: Vec<_> = to_remove.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        println!("  Removed models:    {}", sorted.join(", "));
    }

    if !apply {
        println!("\n[Dry-run] Use --apply to write changes");
        return;
    }

    if to_add.is_empty() && to_remove.is_empty() && changed == 0 {
        println!("\nNo changes needed.");
        return;
    }

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let bak_path = zed_path.with_file_name(format!("settings.json.bak.{}", timestamp));
    fs::copy(zed_path, &bak_path).unwrap_or_else(|e| {
        eprintln!("Backup failed: {}", e);
        process::exit(1);
    });
    println!("\nBackup saved: {}", bak_path.display());

    cfg["language_models"]["openai_compatible"]["kairux"]["available_models"] = json!(new_models);
    fs::write(
        zed_path,
        serde_json::to_string_pretty(&cfg).unwrap_or_else(|e| {
            eprintln!("JSON write error: {}", e);
            process::exit(1);
        }) + "\n",
    )
    .unwrap_or_else(|e| {
        eprintln!("Write error: {}", e);
        process::exit(1);
    });

    println!("Zed config updated: {}", zed_path.display());
    println!("Total models:   {}", new_models.len());
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let apply = args.iter().any(|a| a == "--apply");
    let claude = args.iter().any(|a| a == "--claude");
    let zed = args.iter().any(|a| a == "--zed");

    let config_path = args
        .windows(2)
        .find(|w| w[0] == "--config")
        .map(|w| PathBuf::from(&w[1]).expanduser())
        .unwrap_or_else(|| {
            let default = if claude {
                DEFAULT_CLAUDE_CONFIG
            } else if zed {
                DEFAULT_ZED_CONFIG
            } else {
                DEFAULT_OPENCODE_CONFIG
            };
            PathBuf::from(default).expanduser()
        });

    let api_key_arg = args
        .windows(2)
        .find(|w| w[0] == "--api")
        .map(|w| w[1].clone());

    let baseurl = args
        .windows(2)
        .find(|w| w[0] == "--baseurl")
        .map(|w| w[1].clone())
        .unwrap_or_else(|| "https://ai.bocahdigital.com".to_string());

    let api_key = get_api_key(api_key_arg);
    println!("Fetching config from {}/v1/internal/config/opencode ...", baseurl);
    let remote_cfg = fetch_config(&api_key, &baseurl);
    let remote_models = remote_cfg
        .get("provider")
        .and_then(|p| p.get("Kairux"))
        .and_then(|k| k.get("models"))
        .and_then(|m| m.as_object())
        .map(|m| m.to_owned())
        .unwrap_or_else(serde_json::Map::new);

    if remote_models.is_empty() {
        eprintln!("No models found in API response");
        process::exit(1);
    }

    if claude {
        sync_claude(&remote_models, &config_path, apply, &api_key, &baseurl);
        return;
    }

    if zed {
        sync_zed(&remote_models, &config_path, apply);
        return;
    }

    if !config_path.exists() {
        eprintln!("Config not found: {}", config_path.display());
        process::exit(1);
    }

    println!("Reading config: {}", config_path.display());
    let cfg_str = fs::read_to_string(&config_path).unwrap_or_else(|e| {
        eprintln!("Read error: {}", e);
        process::exit(1);
    });
    let mut cfg: Value = serde_json::from_str(&cfg_str).unwrap_or_else(|e| {
        eprintln!("JSON parse error: {}", e);
        process::exit(1);
    });

    let kairux = cfg
        .get("provider")
        .and_then(|p| p.get("Kairux"))
        .unwrap_or_else(|| {
            eprintln!("No 'Kairux' provider found in config");
            process::exit(1);
        });

    let old_models = kairux
        .get("models")
        .and_then(|m| m.as_object())
        .map(|m| m.to_owned())
        .unwrap_or_else(serde_json::Map::new);

    println!("\nSync summary:");
    println!("  Total in config:   {}", old_models.len());
    println!("  Total from API:    {}", remote_models.len());

    let old_ids: std::collections::HashSet<_> = old_models.keys().collect();
    let new_ids: std::collections::HashSet<_> = remote_models.keys().collect();

    let to_add: Vec<_> = new_ids.difference(&old_ids).collect();
    let to_remove: Vec<_> = old_ids.difference(&new_ids).collect();

    let mut changed = 0;
    for mid in old_ids.intersection(&new_ids) {
        if old_models[*mid] != remote_models[*mid] {
            changed += 1;
        }
    }

    println!("  To add:            {}", to_add.len());
    println!("  To remove:         {}", to_remove.len());
    println!("  Updated:           {}", changed);

    if !to_add.is_empty() {
        let mut sorted: Vec<_> = to_add.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        println!("  New models:        {}", sorted.join(", "));
    }
    if !to_remove.is_empty() {
        let mut sorted: Vec<_> = to_remove.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        println!("  Removed models:    {}", sorted.join(", "));
    }

    if !apply {
        println!("\n[Dry-run] Use --apply to write changes");
        return;
    }

    if to_add.is_empty() && to_remove.is_empty() && changed == 0 {
        println!("\nNo changes needed.");
        return;
    }

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let bak_path = config_path.with_file_name(format!(
        "opencode.json.bak.{}",
        timestamp
    ));
    fs::copy(&config_path, &bak_path).unwrap_or_else(|e| {
        eprintln!("Backup failed: {}", e);
        process::exit(1);
    });
    println!("\nBackup saved: {}", bak_path.display());

    cfg["provider"]["Kairux"]["models"] = json!(remote_models);
    cfg["provider"]["Kairux"]["options"]["baseURL"] = json!(format!("{}/v1", baseurl));
    fs::write(
        &config_path,
        serde_json::to_string_pretty(&cfg).unwrap_or_else(|e| {
            eprintln!("JSON write error: {}", e);
            process::exit(1);
        }) + "\n",
    )
    .unwrap_or_else(|e| {
        eprintln!("Write error: {}", e);
        process::exit(1);
    });

    println!("Config updated: {}", config_path.display());
    println!("Total models:   {}", remote_models.len());
}

trait ExpandTilde {
    fn expanduser(&self) -> PathBuf;
}

impl ExpandTilde for PathBuf {
    fn expanduser(&self) -> PathBuf {
        let s = self.to_string_lossy();
        if s.starts_with("~/") {
            PathBuf::from(env::var("HOME").unwrap_or_default()).join(&s[2..])
        } else {
            self.to_path_buf()
        }
    }
}
