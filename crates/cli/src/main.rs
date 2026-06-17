use clap::{Parser, Subcommand};
use nannou_manager_core::*;

#[derive(Parser)]
#[command(
    name = "nou",
    about = "Manage nannou creative coding projects",
    long_about = "Manage nannou creative coding projects.\n\nRun without a subcommand to launch the Nannou Manager desktop app.",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
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

    /// Delete a project from disk
    Delete {
        /// Project name
        name: String,

        /// Skip the confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Copy a project to a new name
    Copy {
        /// Existing project name
        from: String,
        /// New project name
        to: String,
    },

    /// Clone a project from a git URL into the projects directory
    Clone {
        /// Repository URL
        url: String,
        /// Folder name (defaults to the repo name)
        name: Option<String>,
    },

    /// Git operations on a project
    Git {
        #[command(subcommand)]
        action: GitAction,
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

#[derive(Subcommand)]
enum GitAction {
    /// Show git status for a project
    Status {
        /// Project name
        name: String,
    },

    /// Initialize a git repository in a project
    Init {
        /// Project name
        name: String,
    },

    /// Set (or change) the origin remote URL
    Remote {
        /// Project name
        name: String,
        /// Remote URL
        url: String,
    },

    /// Stage all changes, commit, and push
    Sync {
        /// Project name
        name: String,
        /// Commit message
        #[arg(short, long, default_value = "Update")]
        message: String,
    },

    /// Pull (fast-forward only) from the configured remote
    Pull {
        /// Project name
        name: String,
    },
}

fn resolve_project(config: &Config, name: &str) -> ProjectInfo {
    match scan_projects(&config.projects_dir)
        .into_iter()
        .find(|p| p.name == name)
    {
        Some(p) => p,
        None => {
            eprintln!("Project '{name}' not found in {}", config.projects_dir);
            std::process::exit(1);
        }
    }
}

fn unwrap_or_exit<T>(result: Result<T, String>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

fn name_from_git_url(url: &str) -> String {
    let cleaned = url.trim().trim_end_matches('/').trim_end_matches(".git");
    let cut = cleaned
        .rfind(|c| c == '/' || c == ':')
        .map(|i| i + 1)
        .unwrap_or(0);
    cleaned[cut..].to_string()
}

fn confirm(prompt: &str) -> bool {
    use std::io::{self, Write};
    print!("{prompt} [y/N]: ");
    io::stdout().flush().ok();
    let mut buf = String::new();
    if io::stdin().read_line(&mut buf).is_err() {
        return false;
    }
    matches!(buf.trim(), "y" | "Y" | "yes" | "YES")
}

/// Launch the desktop UI in-process. Present only when built with the
/// `desktop` feature; headless builds get the stub below instead.
#[cfg(feature = "desktop")]
fn launch_gui() {
    nannou_manager_desktop::run();
}

#[cfg(not(feature = "desktop"))]
fn launch_gui() {
    eprintln!(
        "This build of `nou` does not include the desktop UI.\n\
         Reinstall with `cargo install nannou-manager-cli --features desktop`, \
         install the desktop package (`brew install --cask davidgraymi/tap/nannou-manager`), \
         or run a subcommand (try `nou --help`)."
    );
    std::process::exit(2);
}

fn print_git_status(status: &GitStatus) {
    if !status.initialized {
        println!("No git repository");
        return;
    }
    println!(
        "Branch  : {}",
        status.branch.as_deref().unwrap_or("(detached)")
    );
    println!("Remote  : {}", status.remote.as_deref().unwrap_or("(none)"));
    println!(
        "Working : {}",
        if status.dirty { "dirty" } else { "clean" }
    );
    if status.remote.is_some() {
        println!("Ahead   : {}", status.ahead);
        println!("Behind  : {}", status.behind);
    }
}

#[cfg(test)]
mod tests {
    use super::name_from_git_url;

    #[test]
    fn https_url_with_git_suffix() {
        assert_eq!(name_from_git_url("https://github.com/foo/bar.git"), "bar");
    }

    #[test]
    fn https_url_without_git_suffix() {
        assert_eq!(name_from_git_url("https://github.com/foo/bar"), "bar");
    }

    #[test]
    fn ssh_url() {
        assert_eq!(name_from_git_url("git@github.com:foo/bar.git"), "bar");
    }

    #[test]
    fn url_with_trailing_slash() {
        assert_eq!(name_from_git_url("https://github.com/foo/bar.git/"), "bar");
    }

    #[test]
    fn url_with_surrounding_whitespace() {
        assert_eq!(name_from_git_url("  https://github.com/foo/bar.git \n"), "bar");
    }

    #[test]
    fn bare_repo_name() {
        assert_eq!(name_from_git_url("bar.git"), "bar");
    }

    #[test]
    fn nested_path() {
        assert_eq!(
            name_from_git_url("https://gitlab.com/group/sub/proj.git"),
            "proj"
        );
    }
}

fn main() {
    let cli = Cli::parse();
    let config = load_config();

    let Some(command) = cli.command else {
        launch_gui();
        return;
    };

    match command {
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

        Commands::New { name } => {
            unwrap_or_exit(create_project(&config, &name));
            println!("Created project '{name}' in {}", config.projects_dir);
        }

        Commands::Run { name } => {
            let project = resolve_project(&config, &name);
            println!("Running '{name}' (cargo run --release)...");
            let mut child = unwrap_or_exit(spawn_project(&project.path));
            let _ = child.wait();
        }

        Commands::Open { name } => {
            let project = resolve_project(&config, &name);
            unwrap_or_exit(open_in_editor(&config.editor_cmd, &project.path));
            println!("Opened '{name}' in {}", config.editor_cmd);
        }

        Commands::Delete { name, yes } => {
            let project = resolve_project(&config, &name);
            if !yes && !confirm(&format!("Delete '{name}' at {}?", project.path)) {
                println!("Aborted.");
                return;
            }
            unwrap_or_exit(delete_project(&project.path));
            println!("Deleted '{name}'");
        }

        Commands::Copy { from, to } => {
            let project = resolve_project(&config, &from);
            unwrap_or_exit(copy_project(&project.path, &config.projects_dir, &to));
            println!("Copied '{from}' to '{to}'");
        }

        Commands::Clone { url, name } => {
            let target_name = name.unwrap_or_else(|| name_from_git_url(&url));
            if target_name.is_empty() {
                eprintln!("Error: could not determine folder name from URL");
                std::process::exit(1);
            }
            unwrap_or_exit(git_clone(&url, &config.projects_dir, &target_name));
            println!("Cloned '{target_name}' into {}", config.projects_dir);
        }

        Commands::Git { action } => match action {
            GitAction::Status { name } => {
                let project = resolve_project(&config, &name);
                print_git_status(&git_status(&project.path));
            }
            GitAction::Init { name } => {
                let project = resolve_project(&config, &name);
                unwrap_or_exit(git_init(&project.path));
                println!("Initialized git repository in '{name}'");
            }
            GitAction::Remote { name, url } => {
                let project = resolve_project(&config, &name);
                unwrap_or_exit(git_set_remote(&project.path, &url));
                println!("Set origin to '{url}' for '{name}'");
            }
            GitAction::Sync { name, message } => {
                let project = resolve_project(&config, &name);
                unwrap_or_exit(git_sync(&project.path, &message));
                println!("Synced '{name}'");
            }
            GitAction::Pull { name } => {
                let project = resolve_project(&config, &name);
                unwrap_or_exit(git_pull(&project.path));
                println!("Pulled '{name}'");
            }
        },

        Commands::Config { action } => match action {
            ConfigAction::Show => {
                println!("Projects directory : {}", config.projects_dir);
                println!("Editor command     : {}", config.editor_cmd);
                println!("Config file        : {}", config_path().display());
            }
            ConfigAction::SetDir { path } => {
                let mut c = config;
                c.projects_dir = path.clone();
                unwrap_or_exit(save_config(&c));
                println!("Projects directory set to '{path}'");
            }
            ConfigAction::SetEditor { cmd } => {
                let mut c = config;
                c.editor_cmd = cmd.clone();
                unwrap_or_exit(save_config(&c));
                println!("Editor set to '{cmd}'");
            }
        },
    }
}
