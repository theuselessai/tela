use anyhow::Context;
use clap::Parser;
use std::fs;
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
            let manifest_str = std::fs::read_to_string(path.join("manifest.json"))
                .context("failed to read manifest.json")?;
            let manifest = tela_engine::parse_manifest(&manifest_str)?;
            let bundle_path = path.join(&manifest.entry);
            
            // Auto-build if bundle doesn't exist
            if !bundle_path.exists() {
                println!("Bundle not found, building...");
                let status = std::process::Command::new("npm")
                    .arg("run")
                    .arg("build")
                    .current_dir(&path)
                    .status()
                    .context("failed to run npm build")?;
                
                if !status.success() {
                    anyhow::bail!("npm build failed");
                }
            }
            
            let bundle = std::fs::read_to_string(&bundle_path)
                .with_context(|| format!("failed to read {}", bundle_path.display()))?;

            let engine = Engine::new_with_manifest(&manifest.name, &manifest.permissions).await?;
            engine.load_bundle(&bundle).await?;
            println!("Dev mode running. Press Ctrl+C to exit.");
            engine.run().await?;
        }
        Cli::Init { name } => {
            let path = std::path::PathBuf::from(&name);
            
            // Create directories
            fs::create_dir_all(path.join("src"))
                .context("failed to create src directory")?;
            
            // Write manifest.json
            let manifest = serde_json::json!({
                "name": name,
                "version": "0.1.0",
                "entry": "bundle.js"
            });
            fs::write(
                path.join("manifest.json"),
                serde_json::to_string_pretty(&manifest)?
            ).context("failed to write manifest.json")?;
            
            // Write package.json
            let package = serde_json::json!({
                "name": name,
                "private": true,
                "scripts": {
                    "build": "esbuild src/index.jsx --jsx-factory=h --jsx-fragment=Fragment --outfile=bundle.js"
                },
                "devDependencies": {
                    "esbuild": "^0.25.0"
                }
            });
            fs::write(
                path.join("package.json"),
                serde_json::to_string_pretty(&package)?
            ).context("failed to write package.json")?;
            
            // Write src/index.jsx with minimal counter app
            let index_jsx = r#"var initialState = { count: 0 };

function reduce(state, action) {
  switch (action.type) {
    case "increment":
      return { count: state.count + 1 };
    case "decrement":
      return { count: state.count - 1 };
    default:
      return state;
  }
}

var keybindings = {
  normal: {
    j: "increment",
    k: "decrement",
    q: "quit",
  },
};

function view(state, dispatch) {
  return (
    <box border="rounded" title="Counter">
      <layout direction="vertical">
        <text align="center">
          Count: {state.count}
        </text>
        <text align="center" fg="gray">
          j: increment | k: decrement | q: quit
        </text>
      </layout>
    </box>
  );
}
"#;
            fs::write(path.join("src/index.jsx"), index_jsx)
                .context("failed to write src/index.jsx")?;
            
            println!("Created {}/ with the following structure:", name);
            println!("  {}/", name);
            println!("  ├── manifest.json");
            println!("  ├── package.json");
            println!("  └── src/");
            println!("      └── index.jsx");
            println!();
            println!("Next steps:");
            println!("  cd {}", name);
            println!("  npm install");
            println!("  npm run build");
            println!("  tela run .");
        }
    }
    Ok(())
}
