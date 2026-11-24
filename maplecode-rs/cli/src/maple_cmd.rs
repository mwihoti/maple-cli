//! Provider commands stay separate from later product branding.

use clap::{Parser, Subcommand};
use maplecode_core::config::ConfigBuilder;
use maplecode_keyring_store::DefaultKeyringStore;
use maplecode_model_provider_info::maple_credentials;
use maplecode_utils_cli::CliConfigOverrides;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, read};
use std::io::{IsTerminal, Write};
use std::sync::Arc;
use zeroize::Zeroizing;

#[derive(Debug, Parser)]
pub(crate) struct MapleCommand {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Save a key in the OS credential store using a non-echoing prompt.
    Login,
    /// Remove the endpoint's stored key. Environment keys remain under shell control.
    Logout,
    /// Inspect credential configuration (does not prove server authentication).
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Fetch the current tool-capable model catalog from Maple.
    Models {
        #[arg(long)]
        search: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum AuthCommand {
    Status,
}

struct RawMode;
impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

fn prompt_key() -> anyhow::Result<Zeroizing<String>> {
    anyhow::ensure!(
        std::io::stdin().is_terminal() && std::io::stderr().is_terminal(),
        "Interactive login needs a terminal; set MAPLE_API_KEY for non-interactive use"
    );
    eprint!("Maple API key (hidden): ");
    std::io::stderr().flush()?;
    crossterm::terminal::enable_raw_mode()?;
    let guard = RawMode;
    let mut key = Zeroizing::new(String::new());
    let result = loop {
        match read()? {
            Event::Key(event) if event.kind == KeyEventKind::Press => match event.code {
                KeyCode::Enter => break Ok(()),
                KeyCode::Esc => break Err(anyhow::anyhow!("Maple login cancelled")),
                KeyCode::Char('c' | 'C') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                    break Err(anyhow::anyhow!("Maple login cancelled"));
                }
                KeyCode::Backspace => {
                    key.pop();
                }
                KeyCode::Char(c)
                    if !event
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    if key.len() >= 16384 {
                        break Err(anyhow::anyhow!("API key input is too long"));
                    }
                    key.push(c);
                }
                _ => {}
            },
            Event::Paste(pasted) => {
                let pasted = Zeroizing::new(pasted);
                if key.len().saturating_add(pasted.len()) > 16384 {
                    break Err(anyhow::anyhow!("API key input is too long"));
                }
                key.push_str(&pasted);
            }
            _ => {}
        }
    };
    drop(guard);
    eprintln!();
    result?;
    maple_credentials::validate_key(&key).map_err(anyhow::Error::msg)?;
    Ok(key)
}

pub(crate) async fn run(
    command: MapleCommand,
    mut overrides: CliConfigOverrides,
) -> anyhow::Result<()> {
    overrides
        .raw_overrides
        .push("model_provider=\"maple\"".into());
    overrides
        .raw_overrides
        .push("features.api_key_model_discovery=true".into());
    let config = ConfigBuilder::default()
        .cli_overrides(overrides.parse_overrides().map_err(anyhow::Error::msg)?)
        .build()
        .await?;
    let base_url = config
        .model_provider
        .base_url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Maple base URL is missing"))?;
    match command.command {
        Command::Login => {
            let key = prompt_key()?;
            maple_credentials::save(&DefaultKeyringStore, base_url, &key)
                .map_err(anyhow::Error::msg)?;
            eprintln!(
                "Maple key saved in the system credential store. Run `maplecode maple models` to verify access."
            );
            if std::env::var_os("MAPLE_API_KEY").is_some() {
                eprintln!("MAPLE_API_KEY takes precedence over the stored key.");
            }
        }
        Command::Logout => {
            maple_credentials::delete(&DefaultKeyringStore, base_url)
                .map_err(anyhow::Error::msg)?;
            eprintln!("Stored Maple key removed.");
            if std::env::var_os("MAPLE_API_KEY").is_some() {
                eprintln!(
                    "MAPLE_API_KEY is still set; unset it in your shell to stop using that key."
                );
            }
        }
        Command::Auth {
            command: AuthCommand::Status,
        } => {
            let env = std::env::var("MAPLE_API_KEY")
                .ok()
                .filter(|v| !v.trim().is_empty());
            let source = if let Some(key) = env {
                let key = Zeroizing::new(key);
                maple_credentials::validate_key(&key).map_err(anyhow::Error::msg)?;
                Some("MAPLE_API_KEY")
            } else {
                let key = maple_credentials::load(&DefaultKeyringStore, base_url)
                    .map_err(anyhow::Error::msg)?;
                key.map(Zeroizing::new).map(|_| "system credential store")
            };
            if let Some(source) = source {
                println!("Maple key configured via {source}; API access has not been verified.");
            } else {
                println!("No Maple key configured.");
            }
        }
        Command::Models { search, json } => {
            anyhow::ensure!(
                config.model_catalog.is_none(),
                "Remove model_catalog_json to discover models from Maple"
            );
            let auth = maplecode_login::AuthManager::shared_from_config(&config, false).await?;
            // Fail before a cache read when the terminal has no usable Maple key.
            let _credential = config.model_provider.api_key()?.map(Zeroizing::new);
            let models = maplecode_core::build_models_manager(&config, Arc::clone(&auth), None);
            let catalog = models
                .try_raw_model_catalog(
                    maplecode_models_manager::manager::RefreshStrategy::Online,
                    config.http_client_factory(),
                )
                .await?;
            anyhow::ensure!(
                !catalog.models.is_empty(),
                "Maple catalog has no explicitly tool-capable chat models"
            );
            let needle = search.unwrap_or_default().to_lowercase();
            let selected = catalog
                .models
                .into_iter()
                .filter(|m| {
                    m.slug.to_lowercase().contains(&needle)
                        || m.display_name.to_lowercase().contains(&needle)
                })
                .collect::<Vec<_>>();
            if json {
                serde_json::to_writer_pretty(std::io::stdout(), &selected)?;
                println!();
            } else {
                for m in selected {
                    let context = m
                        .context_window
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "unknown".into());
                    // Escape provider-controlled controls before writing terminal text.
                    let id = m.slug.escape_debug().to_string();
                    let name = m.display_name.escape_debug().to_string();
                    println!("{id}\t{name}\ttools=yes\tcontext={context}");
                }
                eprintln!(
                    "Catalog membership does not guarantee account entitlement. Use --model with an exact ID."
                );
            }
        }
    }
    Ok(())
}
