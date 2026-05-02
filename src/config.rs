use crate::client::{AuthMethod, SelectedServer};
use crate::themes::dialoguer::DialogTheme;
use dialoguer::{Confirm, Input, Password};
use dirs::{config_dir, data_dir};
use std::collections::HashMap;
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct AuthEntry {
    pub known_urls: Vec<String>,
    pub device_id: String,
    pub user_id: String,
    pub username: String,
    pub token: String,
    pub salt: String,
}
// ServerId -> AuthEntry
pub type AuthCache = HashMap<String, AuthEntry>;

#[derive(Debug, Clone, Copy)]
pub enum LyricsVisibility {
    Always,
    Auto,
    Never,
}
impl LyricsVisibility {
    pub fn from_config(val: &str) -> Self {
        match val {
            "auto" => Self::Auto,
            "never" => Self::Never,
            _ => Self::Always,
        }
    }
}

/// This makes sure all dirs are created before we do anything.
/// Also makes unwraps on dirs::data_dir and config_dir safe to do. In theory ;)
pub fn prepare_directories() -> Result<(), Box<dyn std::error::Error>> {
    // these are the system-wide dirs like ~/.local/share and ~/config
    let data_dir = data_dir().expect(" ! Failed getting data directory");
    let config_dir = config_dir().expect(" ! Failed getting config directory");

    let app_data_dir = data_dir.join("navidrome-tui");
    let app_config_dir = config_dir.join("navidrome-tui");

    std::fs::create_dir_all(&app_data_dir)?;
    std::fs::create_dir_all(&app_config_dir)?;

    std::fs::create_dir_all(app_data_dir.join("log"))?;
    std::fs::create_dir_all(app_data_dir.join("covers"))?;
    std::fs::create_dir_all(app_data_dir.join("states"))?;
    std::fs::create_dir_all(app_data_dir.join("preferences"))?;
    std::fs::create_dir_all(app_data_dir.join("downloads"))?;
    std::fs::create_dir_all(app_data_dir.join("databases"))?;
    std::fs::create_dir_all(app_data_dir.join("mpv-scripts"))?;

    // deprecated files, remove this at some point!
    let _ = std::fs::remove_file(app_data_dir.join("state.json"));
    let _ = std::fs::remove_file(app_data_dir.join("offline_state.json"));
    let _ = std::fs::remove_file(app_data_dir.join("seen_artists"));
    let _ = std::fs::remove_file(app_data_dir.join("server_map.json"));

    Ok(())
}

pub fn get_config() -> Result<(PathBuf, serde_yaml::Value), Box<dyn std::error::Error>> {
    let config_dir = match config_dir() {
        Some(dir) => dir,
        None => {
            return Err("Could not find config directory".into());
        }
    };

    let config_file: PathBuf = config_dir.join("navidrome-tui").join("config.yaml");

    let f = std::fs::File::open(&config_file)?;
    let d = serde_yaml::from_reader(f)?;

    Ok((config_file, d))
}

pub fn expand_env_vars(value: &str) -> Result<String, String> {
    let chars: Vec<char> = value.chars().collect();
    let mut output = String::with_capacity(value.len());
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '$' if chars.get(i + 1) == Some(&'{') => {
                let start = i + 2;
                let Some(end) = chars[start..].iter().position(|c| *c == '}') else {
                    return Err(format!("Unclosed environment variable in '{}'", value));
                };
                let end = start + end;
                let name: String = chars[start..end].iter().collect();
                output.push_str(&read_env_var(&name)?);
                i = end + 1;
            }
            '$' => {
                let start = i + 1;
                let mut end = start;
                while end < chars.len() && is_env_var_char(chars[end]) {
                    end += 1;
                }
                if end == start {
                    output.push('$');
                    i += 1;
                } else {
                    let name: String = chars[start..end].iter().collect();
                    output.push_str(&read_env_var(&name)?);
                    i = end;
                }
            }
            '%' => {
                let start = i + 1;
                if let Some(end_offset) = chars[start..].iter().position(|c| *c == '%') {
                    let end = start + end_offset;
                    let name: String = chars[start..end].iter().collect();
                    if name.is_empty() || !name.chars().all(is_env_var_char) {
                        output.push('%');
                        i += 1;
                    } else {
                        output.push_str(&read_env_var(&name)?);
                        i = end + 1;
                    }
                } else {
                    output.push('%');
                    i += 1;
                }
            }
            c => {
                output.push(c);
                i += 1;
            }
        }
    }

    Ok(output)
}

fn is_env_var_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn read_env_var(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("Environment variable '{}' is not set", name))
}

pub fn select_server(
    config: &serde_yaml::Value,
    force_server_select: bool,
) -> Option<SelectedServer> {
    let servers =
        config["servers"].as_sequence().expect(" ! Could not find servers in config file");

    if servers.is_empty() {
        println!(" ! No servers configured in config file");
        std::process::exit(1);
    }

    let server = if servers.len() == 1 {
        &servers[0]
    } else if let Some(default) =
        servers.iter().find(|s| s.get("default").and_then(|v| v.as_bool()).unwrap_or(false))
    {
        if !force_server_select {
            println!(
                " - Server: {} [{}] — use --select-server to switch.",
                default["name"].as_str().unwrap_or("Unnamed"),
                default["url"].as_str().unwrap_or("Unknown")
            );
            default
        } else {
            select_server_interactively(servers)?
        }
    } else {
        select_server_interactively(servers)?
    };

    Some(parse_server(server))
}

fn select_server_interactively(servers: &[serde_yaml::Value]) -> Option<&serde_yaml::Value> {
    let mut names: Vec<String> = servers
        .iter()
        .map(|s| {
            format!(
                "{} ({})",
                s["name"].as_str().unwrap_or("Unnamed"),
                s["url"].as_str().unwrap_or("Unknown")
            )
        })
        .collect();
    names.push("Offline Library".to_string());

    let selection = dialoguer::Select::with_theme(&DialogTheme::default())
        .with_prompt("Which server would you like to use?")
        .items(&names)
        .default(0)
        .interact()
        .unwrap_or(0);

    if selection == names.len() - 1 {
        return None;
    }

    Some(&servers[selection])
}

fn parse_server(server: &serde_yaml::Value) -> SelectedServer {
    let url = match server["url"].as_str().map(expand_env_vars) {
        Some(Ok(url)) if !url.ends_with('/') => url,
        Some(Ok(_)) => exit_config_error("Server URL must not end with a trailing slash"),
        Some(Err(e)) => exit_config_error(&e),
        None => {
            println!(" ! Selected server does not have a URL configured");
            std::process::exit(1);
        }
    };

    if server["name"].as_str().is_none() {
        println!(" ! Selected server does not have a name configured");
        std::process::exit(1);
    }

    let auth = match server["username"].as_str() {
        Some(username) => {
            let username = expand_env_vars(username).unwrap_or_else(|e| exit_config_error(&e));
            let password = match (server["password"].as_str(), server["password_file"].as_str()) {
                (None, Some(password_file)) => {
                    let password_file =
                        expand_env_vars(password_file).unwrap_or_else(|e| exit_config_error(&e));
                    std::fs::read_to_string(&password_file)
                        .unwrap_or_else(|e| {
                            println!(" ! Error reading password file '{}': {}", password_file, e);
                            std::process::exit(1);
                        })
                        .trim_matches(['\n', '\r'])
                        .to_string()
                }
                (Some(p), None) => expand_env_vars(p).unwrap_or_else(|e| exit_config_error(&e)),
                (Some(_), Some(_)) => {
                    println!(
                        " ! Selected server has password and password_file configured, only choose one"
                    );
                    std::process::exit(1);
                }
                (None, None) => {
                    println!(" ! Selected server does not have a password configured");
                    std::process::exit(1);
                }
            };

            AuthMethod::UserPass { username, password }
        }
        None => {
            println!(" ! Selected server does not have a username configured");
            std::process::exit(1);
        }
    };

    SelectedServer { url, auth }
}

fn exit_config_error(message: &str) -> ! {
    println!(" ! {}", message);
    std::process::exit(1);
}

pub fn initialize_config() {
    let config_dir = match config_dir() {
        Some(dir) => dir,
        None => {
            println!(" ! Could not find config directory");
            std::process::exit(1);
        }
    };

    let config_file = config_dir.join("navidrome-tui").join("config.yaml");

    let mut updating = false;
    if config_file.exists() {
        // the config file changed this version. Let's check for a servers array, if it doesn't exist we do the following
        // 1. rename old config
        // 2. run the rest of this function to create a new config file and tell the user about it
        if let Ok(content) = std::fs::read_to_string(&config_file) {
            if !content.contains("servers:") && content.contains("server:") {
                updating = true;
                let old_config_file = config_file.with_extension("_old");
                std::fs::rename(&config_file, &old_config_file)
                    .expect(" ! Could not rename old config file");
                println!(
                    " ! Your config file is outdated and has been backed up to: config_old.yaml"
                );
                println!(" ! A new config will now be created. Please go through the setup again.");
                println!(" ! This is done to support the new offline mode and multiple servers.\n");
            }
        }
        if !updating {
            println!(" - Config loaded: {}", config_file.display());
            return;
        }
    }

    let mut server_name = String::new();
    let mut server_url = String::new();
    let mut username = String::new();
    let mut password = String::new();

    println!(" - Thank you for trying navidrome-tui! <3\n");
    println!(" - If you encounter issues or missing features, please report them here:");
    println!(" - https://github.com/leandro754/Navidrome-tui/issues\n");
    println!(" ! Configuration file not found. Please enter the following details:\n");

    let http_client = reqwest::blocking::Client::new();

    let mut ok = false;
    let mut counter = 0;
    while !ok {
        server_url = Input::with_theme(&DialogTheme::default())
            .with_prompt("Server URL")
            .with_initial_text("http://")
            .validate_with({
                move |input: &String| -> Result<(), &str> {
                    if input.starts_with("http://")
                        || input.starts_with("https://")
                            && input != "http://"
                            && input != "https://"
                    {
                        Ok(())
                    } else {
                        Err("Please enter a valid URL including http or https")
                    }
                }
            })
            .interact_text()
            .unwrap();

        if server_url.ends_with('/') {
            server_url.pop();
        }

        server_name = Input::with_theme(&DialogTheme::default())
            .with_prompt("Server name")
            .with_initial_text("Home Server")
            .interact_text()
            .unwrap();

        username = Input::with_theme(&DialogTheme::default())
            .with_prompt("Username")
            .interact_text()
            .unwrap();

        password = Password::with_theme(&DialogTheme::default())
            .allow_empty_password(true)
            .with_prompt("Password")
            .interact()
            .unwrap();

        // Navidrome uses Subsonic token authentication.
        let salt = crate::client::random_string();
        let token = format!("{:x}", md5::compute(format!("{}{}", password, salt)));

        let url: String = format!(
            "{}/rest/ping.view?u={}&t={}&s={}&v=1.16.1&c=navidrome-tui&f=json",
            server_url, username, token, salt
        );
        match http_client.get(&url).send() {
            Ok(response) => {
                if !response.status().is_success() {
                    println!(" ! Connection failed: {}", response.status());
                    continue;
                }
                let value = match response.json::<serde_json::Value>() {
                    Ok(v) => v,
                    Err(e) => {
                        println!(" ! Error parsing response: {}", e);
                        continue;
                    }
                };
                let resp = &value["subsonic-response"];
                if resp.is_null() || resp["status"].as_str() != Some("ok") {
                    println!(" ! Error authenticating: {:?}", resp["error"]);
                    continue;
                }
            }
            Err(e) => {
                println!(" ! Error authenticating: {}", e);
                continue;
            }
        }

        let confirm_prompt = format!(
            "Success! Use server '{}' ({}) as user '{}'?",
            server_name.trim(),
            server_url.trim(),
            username.trim(),
        );

        match Confirm::with_theme(&DialogTheme::default())
            .with_prompt(&confirm_prompt)
            .default(true)
            .wait_for_newline(true)
            .interact_opt()
            .unwrap()
        {
            Some(true) => {
                ok = true;
            }
            _ => {
                counter += 1;
                if counter >= 3 {
                    println!(" I believe in you! You can do it!");
                } else {
                    println!(" ! Let's try again.\n");
                }
            }
        }
    }

    let default_download = dirs::audio_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("navidrome-tui")
        .to_string_lossy()
        .into_owned();

    let download_path: String = Input::with_theme(&DialogTheme::default())
        .with_prompt("Download folder (songs saved as Artist/Album/Artist - Title.ext)")
        .with_initial_text(&default_download)
        .interact_text()
        .unwrap();

    let server_entry = serde_json::json!({
        "name": server_name.trim(),
        "url": server_url.trim(),
        "username": username.trim(),
        "password": password.trim(),
    });

    let default_config = serde_yaml::to_string(&serde_json::json!({
        "servers": [ server_entry ],
        "download_path": download_path.trim(),
    }))
    .expect(" ! Could not serialize default configuration");

    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    opts.mode(0o600);
    let mut file = opts.open(&config_file).expect(" ! Could not create config file");
    file.write_all(default_config.as_bytes()).expect(" ! Could not write default config");

    println!(
        " - Created default config file at: {}",
        config_file.to_str().expect(" ! Could not convert config path to string.")
    );
}

pub fn load_auth_cache() -> Result<AuthCache, Box<dyn std::error::Error>> {
    let path = dirs::data_dir().unwrap().join("navidrome-tui").join("auth_cache.json");
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let content = std::fs::read_to_string(path)?;
    let cache: AuthCache = serde_json::from_str(&content)?;
    Ok(cache)
}

pub fn save_auth_cache(cache: &AuthCache) -> Result<(), Box<dyn std::error::Error>> {
    let path = dirs::data_dir().unwrap().join("navidrome-tui").join("auth_cache.json");
    let json = serde_json::to_string_pretty(cache)?;
    let mut file = {
        let mut opts = OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        opts.mode(0o600);
        opts.open(&path)?
    };
    file.write_all(json.as_bytes())?;
    Ok(())
}

pub fn find_cached_auth_by_url<'a>(
    cache: &'a AuthCache,
    url: &str,
) -> Option<(&'a String, &'a AuthEntry)> {
    for (server_id, entry) in cache {
        if entry.known_urls.contains(&url.to_string()) {
            return Some((server_id, entry));
        }
    }
    None
}

/// This is called after a successful connection.
/// Writes a mapping of (Server from config.yaml) -> (ServerId from navidrome), among other things, to a file.
/// This is later used to show the server name when choosing an offline database.
pub fn update_cache_with_new_auth(
    mut cache: AuthCache,
    selected_server: &SelectedServer,
    client: &crate::client::Client,
) -> AuthCache {
    let server_id = &client.server_id;

    let entry = cache.entry(server_id.clone()).or_insert(AuthEntry {
        known_urls: vec![],
        device_id: client.device_id.clone(),
        user_id: client.user_id.clone(),
        username: client.user_name.clone(),
        token: client.token.clone(),
        salt: client.salt.clone(),
    });

    if !entry.known_urls.contains(&selected_server.url) {
        entry.known_urls.push(selected_server.url.clone());
    }

    entry.user_id = client.user_id.clone();
    entry.username = client.user_name.clone();
    entry.token = client.token.clone();
    entry.salt = client.salt.clone();

    cache
}
