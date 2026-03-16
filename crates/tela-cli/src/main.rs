use anyhow::Context;
use clap::Parser;
use tela_engine::Engine;

#[derive(Parser)]
#[command(name = "tela", about = "React Native for terminals")]
enum Cli {
    /// Run a Tela app
    Run {
        /// Path to the app directory
        path: std::path::PathBuf,
    },
    /// Start dev mode with hot reload
    Dev {
        /// Path to the app directory
        path: std::path::PathBuf,
    },
    /// Create a new Tela app
    Init {
        /// App name
        name: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli {
        Cli::Run { path } => {
            let manifest_str = std::fs::read_to_string(path.join("manifest.json"))
                .context("failed to read manifest.json")?;
            let manifest = tela_engine::parse_manifest(&manifest_str)?;
            let bundle_path = path.join(&manifest.entry);
            let bundle = std::fs::read_to_string(&bundle_path)
                .with_context(|| format!("failed to read {}", bundle_path.display()))?;

            let engine = Engine::new_with_manifest(&manifest.name, &manifest.permissions).await?;
            engine.load_bundle(&bundle).await?;
            engine.run().await?;
        }
        Cli::Dev { path } => {
            println!("Dev mode at: {}", path.display());
            // TODO: watch + hot reload
        }
        Cli::Init { name } => {
            println!("Creating new app: {}", name);
            // TODO: scaffold app
        }
    }
    Ok(())
}
