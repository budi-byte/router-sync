use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

const API_URL: &str = "https://cb.bohongan.com/v1/internal/config/opencode";

fn get_api_key() -> String {
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

fn fetch_config(api_key: &str) -> Value {
    let client = reqwest::blocking::Client::new();
    match client
        .get(API_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("User-Agent", "Mozilla/5.0")
        .send()
    {
        Ok(resp) if resp.status().is_success() => resp.json().unwrap_or_else(|e| {
            eprintln!("JSON parse error: {}", e);
            process::exit(1);
        }),
        Ok(resp) => {
            eprintln!("HTTP error: {}", resp.status());
            process::exit(1);
        }
        Err(e) => {
            eprintln!("Request failed: {}", e);
            process::exit(1);
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let apply = args.iter().any(|a| a == "--apply");
    let config_path = args
        .windows(2)
        .find(|w| w[0] == "--config")
        .map(|w| PathBuf::from(&w[1]).expanduser())
        .unwrap_or_else(|| PathBuf::from("~/.config/opencode/opencode.json").expanduser());

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

    let api_key = get_api_key();
    println!("Fetching config from {} ...", API_URL);
    let remote_cfg = fetch_config(&api_key);
    let remote_models = remote_cfg
        .get("provider")
        .and_then(|p| p.get("Kairux"))
        .and_then(|k| k.get("models"))
        .and_then(|m| m.as_object())
        .map(|m| m.to_owned())
        .unwrap_or_else(serde_json::Map::new);

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
