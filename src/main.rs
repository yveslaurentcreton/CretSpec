use anyhow::{Context, Result, bail};
use clap::{Args, CommandFactory, Parser, Subcommand};
use cretspec::{agents, config, files, operations, skills, workspace};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "cspec",
    version,
    about = "Prepare a complete development project from its specification",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    /// Inspect workspace sources and agent readiness without changing files.
    Context {
        location: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Refresh local instructions and skills from the current project definition.
    Sync {
        location: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Inspect skills or create a skill in its source repository.
    Skill {
        #[command(subcommand)]
        command: SkillCommand,
    },
    /// Configure the default repository namespace.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Clone, inspect or open a project.
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Work with the project's editable guidelines.
    Guidelines {
        #[command(subcommand)]
        command: GuidelinesCommand,
    },
}
#[derive(Subcommand)]
enum SkillCommand {
    /// List active skills and editable shared drafts without changing files.
    List {
        location: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Create a standard skill scaffold without overwriting an existing skill.
    Create {
        name: String,
        #[arg(long, value_enum)]
        scope: skills::Scope,
        #[arg(long)]
        description: String,
        #[arg(long)]
        location: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}
#[derive(Subcommand)]
enum ConfigCommand {
    /// Show or set the GitHub owner, namespace URL or local source directory.
    Namespace { value: Option<String> },
}
#[derive(Subcommand)]
enum ProjectCommand {
    /// Initialize a definition in an existing, uninitialized spec repository.
    Init {
        directory: PathBuf,
        #[arg(long)]
        name: String,
        #[arg(long)]
        code: String,
        #[arg(long)]
        guidelines: String,
        #[arg(long = "ref")]
        reference: Option<String>,
        /// Pin a committed guidelines version instead of using the working tree.
        #[arg(long = "lock", requires = "reference")]
        pinned: bool,
        #[arg(long)]
        profile: String,
    },
    /// Check project health without fetching or changing files.
    Doctor {
        location: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Clone the spec, code and guidelines into one new project root.
    Clone {
        spec: String,
        destination: Option<PathBuf>,
    },
    /// Attach correctly placed existing code and spec repositories.
    Attach { code: PathBuf, spec: PathBuf },
    /// Show the project definition and prepare missing local context.
    Info { location: Option<PathBuf> },
    /// Generate and open an editor workspace inside the spec.
    Open(OpenArgs),
}
#[derive(Subcommand)]
enum GuidelinesCommand {
    /// Use the editable guidelines branch directly, preserving all local changes.
    Unlock {
        location: Option<PathBuf>,
        #[arg(long)]
        preview: bool,
    },
    /// Preview or adopt an exact guidelines version, preserving local drafts.
    Update {
        reference: String,
        location: Option<PathBuf>,
        #[arg(long)]
        preview: bool,
        #[arg(long)]
        fetch: bool,
    },
    /// Restore the original definition after an interrupted guidelines update.
    Recover { location: Option<PathBuf> },
    /// Print the editable guidelines directory.
    Path { location: Option<PathBuf> },
    /// Open the editable guidelines directory.
    Edit(OpenArgs),
}
#[derive(Args)]
struct OpenArgs {
    location: Option<PathBuf>,
    #[arg(long)]
    print: bool,
}

fn location(value: Option<PathBuf>) -> Result<PathBuf> {
    files::absolute(value.unwrap_or_else(|| PathBuf::from(".")))
}
fn progress(message: &str) {
    eprintln!("{message}");
}

fn execute(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Context {
            location: value,
            json,
        } => {
            let info = workspace::info(&location(value)?, false, &progress)?;
            let context = agents::context(&info)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&context)?);
            } else {
                println!(
                    "{}\nSpec: {}\nCode: {}\nShared skill source: {}\nGuidelines: {:?}, {} ({})\nAgent context: {}",
                    info.manifest.name,
                    info.spec.display(),
                    info.code.display(),
                    info.editable_guidelines.join("skills").display(),
                    info.guidelines.mode,
                    info.manifest.guidelines.reference,
                    &info.guidelines.commit[..12],
                    context["integration"]["detail"]
                        .as_str()
                        .unwrap_or_default()
                );
            }
        }
        Commands::Sync {
            location: value,
            json,
        } => {
            let info = workspace::info(&location(value)?, false, &progress)?;
            let result = agents::sync(&info)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!(
                    "Agent context synchronized: {} written, {} removed, {} unchanged.",
                    result.written, result.removed, result.unchanged
                );
                if let Some(notice) = result.compatibility_notice {
                    println!("{notice}");
                }
            }
        }
        Commands::Skill { command } => match command {
            SkillCommand::List {
                location: value,
                json,
            } => {
                let info = workspace::info(&location(value)?, false, &progress)?;
                let inventory = skills::inventory(&info)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&inventory)?);
                } else {
                    for skill in inventory.active {
                        println!("{} ({:?}): {}", skill.name, skill.scope, skill.description);
                    }
                    for skill in inventory.shared_drafts {
                        println!(
                            "{} (shared working copy): {}",
                            skill.name,
                            skill.source.unwrap().display()
                        );
                    }
                    for error in inventory.draft_errors {
                        eprintln!("Shared draft issue: {error}");
                    }
                }
            }
            SkillCommand::Create {
                name,
                scope,
                description,
                location: value,
                json,
            } => {
                let info = workspace::info(&location(value)?, false, &progress)?;
                let source = skills::create(&info, &name, scope, &description)?;
                let next = if scope == skills::Scope::Project || info.lock.is_none() {
                    "Complete SKILL.md and its resources, then run cspec sync."
                } else {
                    "Complete the shared source, publish it through your Git workflow, then explicitly adopt that guidelines version."
                };
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &serde_json::json!({"schemaVersion":1,"name":name,"scope":scope,"source":source,"nextStep":next})
                        )?
                    );
                } else {
                    println!("Skill source created: {}\n{next}", source.display());
                }
            }
        },
        Commands::Config {
            command: ConfigCommand::Namespace { value },
        } => {
            let namespace=match value { Some(value)=>config::set_namespace(&value)?, None=>config::get_namespace()?.context("No repository namespace configured. Use cspec config namespace <GitHub-owner-or-namespace-URL>")? };
            println!("{namespace}");
        }
        Commands::Project { command } => match command {
            ProjectCommand::Init {
                directory,
                name,
                code,
                guidelines,
                reference,
                pinned,
                profile,
            } => {
                let spec = operations::initialize(operations::InitOptions {
                    directory: &directory,
                    name: &name,
                    code: &code,
                    guidelines: &guidelines,
                    reference: reference.as_deref(),
                    pinned,
                    profile: &profile,
                })?;
                println!(
                    "Specification initialized: {}\nReview the spec, then commit and push it before cloning from its remote.",
                    spec.display()
                );
            }
            ProjectCommand::Doctor {
                location: value,
                json,
            } => {
                let checks = operations::diagnose(&location(value)?);
                if json {
                    println!("{}", serde_json::to_string_pretty(&checks)?);
                } else {
                    for check in &checks {
                        println!(
                            "{} {}: {}",
                            if check.ok { "OK" } else { "FAIL" },
                            check.name,
                            check.detail
                        );
                    }
                }
                if checks.iter().any(|check| !check.ok) {
                    bail!("Project checks failed. Follow the reported next steps.");
                }
            }
            ProjectCommand::Clone { spec, destination } => {
                let namespace = if files::portable_name(&spec) {
                    config::get_namespace()?
                } else {
                    None
                };
                let info = workspace::clone_project(
                    &spec,
                    destination.as_deref(),
                    namespace.as_deref(),
                    &std::env::current_dir()?,
                    &progress,
                )?;
                println!(
                    "{} is ready.\nProject: {}\nSpec: {}\nCode: {}\nEditable guidelines: {}\nActive guidelines: {} ({})",
                    info.manifest.name,
                    info.project_root.display(),
                    info.spec.display(),
                    info.code.display(),
                    info.editable_guidelines.display(),
                    info.manifest.guidelines.reference,
                    &info.guidelines.commit[..12]
                );
            }
            ProjectCommand::Attach { code, spec } => {
                let info = workspace::attach(&code, &spec, &progress)?;
                println!("Project ready: {}", info.project_root.display());
            }
            ProjectCommand::Info { location: value } => {
                let info = workspace::info(&location(value)?, true, &progress)?;
                agents::sync(&info)?;
                println!("{}", serde_json::to_string_pretty(&info)?);
            }
            ProjectCommand::Open(args) => {
                let path = workspace::editor_workspace(&location(args.location)?)?;
                println!("{}", path.display());
                if !args.print {
                    workspace::open_path(&path)?;
                }
            }
        },
        Commands::Guidelines { command } => match command {
            GuidelinesCommand::Unlock {
                location: value,
                preview,
            } => {
                let info = operations::unlock(&location(value)?, preview)?;
                println!(
                    "{}: working-tree guidelines on {} at {}. Use Git to share or retrieve changes.",
                    if preview {
                        "Preview"
                    } else {
                        "Guidelines unlocked"
                    },
                    info.manifest.guidelines.reference,
                    info.editable_guidelines.display()
                );
                if preview {
                    println!("The manifest and lock were not changed.");
                }
            }
            GuidelinesCommand::Update {
                reference,
                location: value,
                preview,
                fetch,
            } => {
                let result = operations::update(&location(value)?, &reference, preview, fetch)?;
                println!(
                    "{}: {:?} ({}) -> pinned {} ({})",
                    if preview {
                        "Preview"
                    } else {
                        "Guidelines selected"
                    },
                    result.previous.mode,
                    &result.previous.commit[..12],
                    result.selected.reference,
                    &result.selected.commit[..12]
                );
                if !result.changes.is_empty() {
                    println!("{}", result.changes);
                }
                if preview {
                    println!("The manifest and lock were not changed.");
                }
            }
            GuidelinesCommand::Recover { location: value } => {
                println!(
                    "Original definition restored: {}",
                    operations::recover(&location(value)?)?.display()
                );
            }
            GuidelinesCommand::Path { location: value } => println!(
                "{}",
                workspace::info(&location(value)?, true, &progress)?
                    .editable_guidelines
                    .display()
            ),
            GuidelinesCommand::Edit(args) => {
                let path = workspace::info(&location(args.location)?, true, &progress)?
                    .editable_guidelines;
                println!("{}", path.display());
                if !args.print {
                    workspace::open_path(&path)?;
                }
            }
        },
    }
    Ok(())
}

fn main() {
    if std::env::args_os().len() == 1 {
        if let Err(error) = Cli::command().print_help() {
            eprintln!("Error: {error}");
            std::process::exit(1);
        }
        println!();
        return;
    }
    if let Err(error) = execute(Cli::parse()) {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}
