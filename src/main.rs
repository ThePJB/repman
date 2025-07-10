use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use colored::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::process::Command as AsyncCommand;

#[derive(Parser)]
#[command(name = "repman")]
#[command(about = "A repository manager for organizing and managing multiple git repositories")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Clone a repository and navigate to it
    Add {
        /// Repository owner/organization
        owner: String,
        /// Repository name
        repo: String,
    },
    /// Show status of all repositories
    Status,
    /// Sync a repository (add, commit, push)
    Sync {
        /// Repository name (directory name)
        name: String,
        /// Commit message
        #[arg(short, long)]
        message: String,
    },
    /// Navigate to a repository directory
    Cd {
        /// Repository name or owner/repo format
        name: String,
    },
}

fn get_repo_root() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
    Ok(home.join("repo"))
}

fn ensure_repo_root_exists() -> Result<PathBuf> {
    let repo_root = get_repo_root()?;
    if !repo_root.exists() {
        fs::create_dir_all(&repo_root)?;
        println!("Created repository root directory: {}", repo_root.display());
    }
    Ok(repo_root)
}

async fn clone_repository(owner: &str, repo: &str) -> Result<()> {
    let repo_root = ensure_repo_root_exists()?;
    let owner_dir = repo_root.join(owner);
    let repo_dir = owner_dir.join(repo);

    if repo_dir.exists() {
        println!("{} Repository already exists at: {}", "✓".green(), repo_dir.display());
        return Ok(());
    }

    // Create owner directory if it doesn't exist
    if !owner_dir.exists() {
        fs::create_dir_all(&owner_dir)?;
    }

    let repo_url = format!("git@github.com:{}/{}.git", owner, repo);
    println!("Cloning {} to {}...", repo_url.cyan(), repo_dir.display());

    let output = AsyncCommand::new("git")
        .args(&["clone", &repo_url, repo_dir.to_str().unwrap()])
        .output()
        .await?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Failed to clone repository: {}", error));
    }

    println!("{} Successfully cloned to: {}", "✓".green(), repo_dir.display());
    println!("Navigate to: {}", format!("cd {}", repo_dir.display()).yellow());
    
    Ok(())
}

fn get_git_status(repo_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(&["status", "--porcelain", "--branch"])
        .current_dir(repo_path)
        .output()?;

    if !output.status.success() {
        return Ok("Not a git repository".to_string());
    }

    let status_output = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = status_output.lines().collect();
    
    if lines.is_empty() {
        return Ok("Clean".green().to_string());
    }

    // Check for ahead/behind status
    if let Some(branch_line) = lines.first() {
        if branch_line.contains("[ahead") {
            return Ok("Ahead".red().to_string());
        } else if branch_line.contains("[behind") {
            return Ok("Behind".yellow().to_string());
        }
    }

    // Check for uncommitted changes
    let has_changes = lines.iter().skip(1).any(|line| !line.trim().is_empty());
    if has_changes {
        Ok("Dirty".red().to_string())
    } else {
        Ok("Clean".green().to_string())
    }
}

async fn show_status() -> Result<()> {
    let repo_root = get_repo_root()?;
    if !repo_root.exists() {
        println!("Repository root directory does not exist: {}", repo_root.display());
        return Ok(());
    }

    println!("{}", "Repository Status:".bold());
    println!();

    let mut found_repos = false;

    // Walk through owner directories
    for owner_entry in fs::read_dir(&repo_root)? {
        let owner_entry = owner_entry?;
        let owner_path = owner_entry.path();
        
        if !owner_path.is_dir() {
            continue;
        }

        let owner_name = owner_path.file_name().unwrap().to_string_lossy();

        // Walk through repository directories
        for repo_entry in fs::read_dir(&owner_path)? {
            let repo_entry = repo_entry?;
            let repo_path = repo_entry.path();
            
            if !repo_path.is_dir() {
                continue;
            }

            let repo_name = repo_path.file_name().unwrap().to_string_lossy();
            let status = get_git_status(&repo_path).unwrap_or_else(|_| "Error".red().to_string());
            
            println!("{}/{} - {}", owner_name.cyan(), repo_name.bold(), status);
            found_repos = true;
        }
    }

    if !found_repos {
        println!("No repositories found in {}", repo_root.display());
    }

    Ok(())
}

async fn sync_repository(name: &str, message: &str) -> Result<()> {
    let repo_root = get_repo_root()?;
    
    // Find the repository by name (search in all owner directories)
    let mut repo_path = None;
    
    for owner_entry in fs::read_dir(&repo_root)? {
        let owner_entry = owner_entry?;
        let owner_path = owner_entry.path();
        
        if !owner_path.is_dir() {
            continue;
        }

        let potential_repo = owner_path.join(name);
        if potential_repo.exists() && potential_repo.is_dir() {
            repo_path = Some(potential_repo);
            break;
        }
    }

    let repo_path = repo_path.ok_or_else(|| anyhow!("Repository '{}' not found", name))?;
    
    println!("Syncing repository: {}", repo_path.display());

    // Git add *
    println!("Adding all changes...");
    let add_output = AsyncCommand::new("git")
        .args(&["add", "."])
        .current_dir(&repo_path)
        .output()
        .await?;

    if !add_output.status.success() {
        let error = String::from_utf8_lossy(&add_output.stderr);
        return Err(anyhow!("Failed to add changes: {}", error));
    }

    // Check if there are changes to commit
    let status_output = AsyncCommand::new("git")
        .args(&["diff", "--cached", "--quiet"])
        .current_dir(&repo_path)
        .output()
        .await?;

    if status_output.status.success() {
        println!("{} No changes to commit", "ℹ".blue());
        return Ok(());
    }

    // Git commit
    println!("Committing with message: '{}'", message);
    let commit_output = AsyncCommand::new("git")
        .args(&["commit", "-m", message])
        .current_dir(&repo_path)
        .output()
        .await?;

    if !commit_output.status.success() {
        let error = String::from_utf8_lossy(&commit_output.stderr);
        return Err(anyhow!("Failed to commit: {}", error));
    }

    // Git push
    println!("Pushing to remote...");
    let push_output = AsyncCommand::new("git")
        .args(&["push"])
        .current_dir(&repo_path)
        .output()
        .await?;

    if !push_output.status.success() {
        let error = String::from_utf8_lossy(&push_output.stderr);
        return Err(anyhow!("Failed to push: {}", error));
    }

    println!("{} Successfully synced repository!", "✓".green());
    Ok(())
}

async fn cd_repository(name: &str) -> Result<()> {
    let repo_root = get_repo_root()?;
    if !repo_root.exists() {
        println!("Repository root directory does not exist: {}", repo_root.display());
        return Ok(());
    }

    // Check if it's owner/repo format
    if name.contains('/') {
        let parts: Vec<&str> = name.split('/').collect();
        if parts.len() == 2 {
            let owner = parts[0];
            let repo = parts[1];
            let repo_path = repo_root.join(owner).join(repo);
            
            if repo_path.exists() {
                println!("cd {}", repo_path.display());
                return Ok(());
            } else {
                println!("{} Repository not found: {}", "✗".red(), repo_path.display());
                return Ok(());
            }
        }
    }

    // Search for repositories matching the name
    let mut matches = Vec::new();
    
    for owner_entry in fs::read_dir(&repo_root)? {
        let owner_entry = owner_entry?;
        let owner_path = owner_entry.path();
        
        if !owner_path.is_dir() {
            continue;
        }

        let owner_name = owner_path.file_name().unwrap().to_string_lossy();

        for repo_entry in fs::read_dir(&owner_path)? {
            let repo_entry = repo_entry?;
            let repo_path = repo_entry.path();
            
            if !repo_path.is_dir() {
                continue;
            }

            let repo_name = repo_path.file_name().unwrap().to_string_lossy();
            
            // Exact match
            if repo_name == name {
                matches.push((owner_name.to_string(), repo_name.to_string(), repo_path.clone()));
            }
            // Fuzzy match (contains)
            else if repo_name.to_lowercase().contains(&name.to_lowercase()) {
                matches.push((owner_name.to_string(), repo_name.to_string(), repo_path.clone()));
            }
        }
    }

    match matches.len() {
        0 => {
            println!("{} No repositories found matching '{}'", "✗".red(), name);
        }
        1 => {
            let (_, _, path) = &matches[0];
            println!("cd {}", path.display());
        }
        _ => {
            println!("{} Multiple repositories found:", "ℹ".blue());
            for (i, (owner, repo, path)) in matches.iter().enumerate() {
                println!("  {}: {}/{} -> {}", i + 1, owner.cyan(), repo.bold(), path.display());
            }
            println!("\nUse the full format: repman cd owner/repo");
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Add { owner, repo } => {
            clone_repository(&owner, &repo).await?;
        }
        Commands::Status => {
            show_status().await?;
        }
        Commands::Sync { name, message } => {
            sync_repository(&name, &message).await?;
        }
        Commands::Cd { name } => {
            cd_repository(&name).await?;
        }
    }

    Ok(())
}
