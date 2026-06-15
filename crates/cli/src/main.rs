use clap::{Parser, Subcommand};
use nannou_manager_core::*;

#[derive(Parser)]
#[command(
    name = "nannou-manager",
    about = "Manage nannou creative coding projects",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all projects
    List,

    /// Create a new nannou project
    New {
        /// Project name
        name: String,
    },

    /// Run a project (blocks until it exits)
    Run {
        /// Project name
        name: String,
    },

    /// Open a project in the configured editor
    Open {
        /// Project name
        name: String,
    },

    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,

    /// Set the projects directory
    SetDir {
        /// Directory path
        path: String,
    },

    /// Set the editor command
    SetEditor {
        /// Editor command (e.g. "code", "zed", "vim")
        cmd: String,
    },
}

fn main() {
    let cli = Cli::parse();
    let config = load_config();

    match cli.command {
        Commands::List => {
            let projects = scan_projects(&config.projects_dir);
            if projects.is_empty() {
                println!("No projects found in {}", config.projects_dir);
            } else {
                for p in &projects {
                    println!("{}", p.name);
                }
            }
        }

        Commands::New { name } => match create_project(&config, &name) {
            Ok(()) => println!("Created project '{name}' in {}", config.projects_dir),
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        },

        Commands::Run { name } => {
            let projects = scan_projects(&config.projects_dir);
            let Some(project) = projects.iter().find(|p| p.name == name) else {
                eprintln!("Project '{name}' not found in {}", config.projects_dir);
                std::process::exit(1);
            };
            println!("Running '{name}' (cargo run --release)...");
            match spawn_project(&project.path) {
                Ok(mut child) => {
                    let _ = child.wait();
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        }

        Commands::Open { name } => {
            let projects = scan_projects(&config.projects_dir);
            let Some(project) = projects.iter().find(|p| p.name == name) else {
                eprintln!("Project '{name}' not found in {}", config.projects_dir);
                std::process::exit(1);
            };
            match open_in_editor(&config.editor_cmd, &project.path) {
                Ok(()) => println!("Opened '{name}' in {}", config.editor_cmd),
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        }

        Commands::Config { action } => match action {
            ConfigAction::Show => {
                println!("Projects directory : {}", config.projects_dir);
                println!("Editor command     : {}", config.editor_cmd);
                println!("Config file        : {}", config_path().display());
            }
            ConfigAction::SetDir { path } => {
                let mut c = config;
                c.projects_dir = path.clone();
                match save_config(&c) {
                    Ok(()) => println!("Projects directory set to '{path}'"),
                    Err(e) => {
                        eprintln!("Error: {e}");
                        std::process::exit(1);
                    }
                }
            }
            ConfigAction::SetEditor { cmd } => {
                let mut c = config;
                c.editor_cmd = cmd.clone();
                match save_config(&c) {
                    Ok(()) => println!("Editor set to '{cmd}'"),
                    Err(e) => {
                        eprintln!("Error: {e}");
                        std::process::exit(1);
                    }
                }
            }
        },
    }
}
