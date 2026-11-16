mod model;
mod store;

use clap::{Args, Parser, Subcommand};
use model::{Project, Task};
use quokkadb::QuokkaId;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use std::process;
use store::{Store, StoreError};

const DEFAULT_DATABASE_PATH: &str = "./data/quokka-tasks";

#[derive(Debug, Parser)]
#[command(
    name = "quokka-tasks",
    version,
    about = "A small personal project and task tracker built with QuokkaDB"
)]
struct Cli {
    /// Path to the local QuokkaDB directory.
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        default_value = DEFAULT_DATABASE_PATH
    )]
    db: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a project or manage project commands.
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Add or complete a task.
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    /// List all projects, optionally filtered by tag.
    List(ListArgs),
    /// Show unfinished tasks grouped by project.
    Tasks(TasksArgs),
    /// Add the small built-in example dataset.
    Demo,
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    /// Create a project.
    New(NewProjectArgs),
}

#[derive(Debug, Subcommand)]
enum TaskCommand {
    /// Append a task to a project.
    Add(AddTaskArgs),
    /// Mark one task complete.
    Done(DoneTaskArgs),
}

#[derive(Debug, Args)]
struct NewProjectArgs {
    /// Project name.
    name: String,
    /// Optional project description.
    #[arg(long, value_name = "TEXT")]
    description: Option<String>,
    /// Project tag; may be supplied more than once.
    #[arg(long = "tag", value_name = "TAG")]
    tags: Vec<String>,
}

#[derive(Debug, Args)]
struct ProjectSelector {
    /// Exact project name.
    #[arg(required_unless_present = "project_id", conflicts_with = "project_id")]
    project: Option<String>,
    /// Select a project by ID when existing projects share a name.
    #[arg(long)]
    project_id: Option<String>,
}

impl ProjectSelector {
    fn resolve(&self, store: &Store) -> AppResult<Project> {
        if let Some(id) = &self.project_id {
            Ok(store.project_by_id(parse_id(id, "project")?)?)
        } else {
            Ok(store.project_by_name(self.project.as_deref().expect("clap requires a project"))?)
        }
    }
}

#[derive(Debug, Args)]
#[command(allow_missing_positional = true)]
struct AddTaskArgs {
    #[command(flatten)]
    selector: ProjectSelector,
    /// Task title.
    title: String,
    /// Priority from 1 (highest) to 3 (lowest).
    #[arg(long, default_value_t = 2, value_parser = parse_priority)]
    priority: u8,
}

#[derive(Debug, Args)]
#[command(allow_missing_positional = true, allow_negative_numbers = true)]
struct DoneTaskArgs {
    #[command(flatten)]
    selector: ProjectSelector,
    /// One-based task number shown by list.
    #[arg(allow_negative_numbers = true)]
    number: String,
}

#[derive(Debug, Args)]
struct ListArgs {
    /// Only show projects containing this tag.
    #[arg(long)]
    tag: Option<String>,
}

#[derive(Debug, Args)]
struct TasksArgs {
    /// Required selector for this command.
    #[arg(long, required = true)]
    unfinished: bool,
}

#[derive(Debug)]
enum AppError {
    Message(String),
    Store(StoreError),
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(message) => formatter.write_str(message),
            Self::Store(error) => error.fmt(formatter),
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Message(_) => None,
            Self::Store(error) => Some(error),
        }
    }
}

impl From<StoreError> for AppError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

type AppResult<T> = Result<T, AppError>;

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("quokka-tasks: {error}");
        process::exit(1);
    }
}

fn run(cli: Cli) -> AppResult<()> {
    let store = Store::open(&cli.db)?;

    match cli.command {
        Command::Project {
            command: ProjectCommand::New(args),
        } => {
            let project = store.create_project(args.name, args.description, args.tags)?;
            println!("Created project “{}”.", project.name);
        }
        Command::Task {
            command: TaskCommand::Add(args),
        } => {
            let project = args.selector.resolve(&store)?;
            let task = store.add_task(project.id, args.title, args.priority)?;
            println!("Added “{}” to “{}”.", task.title, project.name);
        }
        Command::Task {
            command: TaskCommand::Done(args),
        } => {
            let project = args.selector.resolve(&store)?;
            let count = project.tasks.len();
            if count == 0 {
                return Err(AppError::Message(format!(
                    "project “{}” has no tasks",
                    project.name
                )));
            }
            let index = args
                .number
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=count).contains(n))
                .ok_or_else(|| {
                    AppError::Message(format!(
                        "choose a task number from 1 to {count} for “{}”",
                        project.name
                    ))
                })?
                - 1;
            let task = &project.tasks[index];
            if task.completed {
                println!("“{}” is already complete.", task.title);
            } else {
                store.complete_task(&project, index)?;
                println!("Completed “{}”.", task.title);
            }
        }
        Command::List(args) => print_projects(&store.list_projects(args.tag.as_deref())?),
        Command::Tasks(args) => {
            if args.unfinished {
                print_unfinished_projects(&store.unfinished_projects()?);
            }
        }
        Command::Demo => {
            let created = store.seed_demo()?;
            if created == 0 {
                println!("Demo data is already present.");
            } else {
                println!("Added {created} demo project(s).");
            }
        }
    }

    Ok(())
}

fn parse_priority(value: &str) -> Result<u8, String> {
    let priority = value
        .parse::<u8>()
        .map_err(|_| "priority must be 1, 2, or 3".to_owned())?;
    if (1..=3).contains(&priority) {
        Ok(priority)
    } else {
        Err("priority must be 1, 2, or 3".to_owned())
    }
}

fn parse_id(value: &str, kind: &str) -> AppResult<QuokkaId> {
    let number = value.parse::<u64>().map_err(|_| {
        AppError::Message(format!(
            "invalid {kind} ID {value:?}; expected a non-negative integer"
        ))
    })?;
    QuokkaId::try_from(number).map_err(|_| {
        AppError::Message(format!(
            "invalid {kind} ID {value:?}; it is too large for a QuokkaDB ID"
        ))
    })
}

fn print_projects(projects: &[Project]) {
    if projects.is_empty() {
        println!("No projects found.");
        return;
    }

    for project in projects {
        print_project(project, false);
    }
}

fn print_unfinished_projects(projects: &[Project]) {
    if projects.is_empty() {
        println!("No unfinished tasks found.");
        return;
    }

    for project in projects {
        print_project(project, true);
    }
}

fn print_project(project: &Project, unfinished_only: bool) {
    println!("{}", project.name);
    if let Some(description) = &project.description {
        println!("  {description}");
    }
    if !project.tags.is_empty() {
        println!("  tags: {}", project.tags.join(", "));
    }

    for (index, task) in project.tasks.iter().enumerate() {
        if unfinished_only && task.completed {
            continue;
        }
        print_task(index + 1, task);
    }
    println!();
}

fn print_task(number: usize, task: &Task) {
    let marker = if task.completed { "x" } else { " " };
    let priority = match task.priority {
        1 => " (high)",
        3 => " (low)",
        _ => "",
    };
    println!("  {number}. [{marker}] {}{priority}", task.title);
}
