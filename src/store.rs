use crate::model::{Project, Task};
use quokkadb::{QuokkaDB, QuokkaId, TypedCollection};
use std::error::Error;
use std::fmt;
use std::path::Path;

const PROJECTS_COLLECTION: &str = "projects";

/// Errors raised by project persistence operations.
#[derive(Debug)]
pub enum StoreError {
    Database(quokkadb::Error),
    ProjectNotFound(QuokkaId),
    Message(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database error: {error}"),
            Self::ProjectNotFound(project_id) => {
                write!(formatter, "project {} was not found", project_id.as_u64())
            }
            Self::Message(message) => formatter.write_str(message),
        }
    }
}

impl Error for StoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::ProjectNotFound(_) | Self::Message(_) => None,
        }
    }
}

impl From<quokkadb::Error> for StoreError {
    fn from(error: quokkadb::Error) -> Self {
        Self::Database(error)
    }
}

/// The persistence boundary for Quokka Tasks.
pub struct Store {
    projects: TypedCollection<Project>,
}

impl Store {
    /// Opens the local database and prepares the projects collection.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let db = QuokkaDB::open(path)?;
        let projects = db
            .typed_collection::<Project>(PROJECTS_COLLECTION)
            .create_if_missing();

        Ok(Self { projects })
    }

    pub fn project_by_id(&self, id: QuokkaId) -> Result<Project, StoreError> {
        self.projects
            .find_one_by_id(id)?
            .ok_or(StoreError::ProjectNotFound(id))
    }

    pub fn project_by_name(&self, name: &str) -> Result<Project, StoreError> {
        let mut matches = self
            .projects
            .find(|p| p.name.eq(name))
            .sort(|p| p.id.asc())
            .execute_collect()?;
        match matches.len() {
            0 => Err(StoreError::Message(format!(
                "project “{name}” was not found"
            ))),
            1 => Ok(matches.pop().unwrap()),
            _ => {
                let options = matches
                    .iter()
                    .map(|p| format!("--project-id {}", p.id.as_u64()))
                    .collect::<Vec<_>>()
                    .join(" or ");
                Err(StoreError::Message(format!(
                    "multiple projects named “{name}”. Use {options}."
                )))
            }
        }
    }

    /// Creates and persists one project.
    pub fn create_project(
        &self,
        name: String,
        description: Option<String>,
        tags: Vec<String>,
    ) -> Result<Project, StoreError> {
        if self.projects.find_one(|p| p.name.eq(&name))?.is_some() {
            return Err(StoreError::Message(format!(
                "project “{name}” already exists"
            )));
        }
        let project = Project {
            id: QuokkaId::new(),
            name,
            description,
            tags,
            tasks: Vec::new(),
        };
        self.projects.insert_one(&project)?;
        Ok(project)
    }

    /// Appends one task with a targeted typed array update.
    pub fn add_task(
        &self,
        project_id: QuokkaId,
        title: String,
        priority: u8,
    ) -> Result<Task, StoreError> {
        let task = Task {
            title,
            completed: false,
            priority,
        };
        let result = self.projects.update_one(
            |project| project.id.eq(project_id),
            |project| project.tasks.push(task.clone()),
        )?;

        if result.matched_count == 0 {
            return Err(StoreError::ProjectNotFound(project_id));
        }
        Ok(task)
    }

    /// Marks one task complete with a targeted nested array update.
    pub fn complete_task(&self, project: &Project, index: usize) -> Result<(), StoreError> {
        let result = self.projects.update_one(
            |p| p.id.eq(project.id),
            |p| p.tasks.at(index).completed.set(true),
        )?;
        if result.matched_count == 0 {
            return Err(StoreError::ProjectNotFound(project.id));
        }
        Ok(())
    }

    /// Lists projects, optionally filtering by a tag in the database query.
    pub fn list_projects(&self, tag: Option<&str>) -> Result<Vec<Project>, StoreError> {
        let projects = match tag {
            Some(tag) => self
                .projects
                .find(|project| project.tags.any_eq(tag))
                .sort(|project| project.name.asc().then(project.id.asc()))
                .execute_collect()?,
            None => self
                .projects
                .find_all()
                .sort(|project| project.name.asc().then(project.id.asc()))
                .execute_collect()?,
        };
        Ok(projects)
    }

    /// Finds projects containing unfinished tasks in the database query.
    pub fn unfinished_projects(&self) -> Result<Vec<Project>, StoreError> {
        Ok(self
            .projects
            .find(|project| project.tasks.any(|task| task.completed.eq(false)))
            .sort(|project| project.name.asc().then(project.id.asc()))
            .execute_collect()?)
    }

    /// Adds the built-in example projects unless their names already exist.
    pub fn seed_demo(&self) -> Result<usize, StoreError> {
        let mut created = 0;

        for project in demo_projects() {
            if self
                .projects
                .find_one(|existing| existing.name.eq(&project.name))?
                .is_some()
            {
                continue;
            }

            self.projects.insert_one(&project)?;
            created += 1;
        }

        Ok(created)
    }
}

fn demo_projects() -> Vec<Project> {
    vec![
        Project {
            id: QuokkaId::new(),
            name: "QuokkaDB launch".to_owned(),
            description: Some("A small release checklist for the QuokkaDB launch.".to_owned()),
            tags: vec!["oss".to_owned(), "release".to_owned()],
            tasks: vec![
                Task {
                    title: "Write announcement".to_owned(),
                    completed: false,
                    priority: 1,
                },
                Task {
                    title: "Publish crate".to_owned(),
                    completed: true,
                    priority: 2,
                },
            ],
        },
        Project {
            id: QuokkaId::new(),
            name: "CLI polish".to_owned(),
            description: Some("Small improvements that make a command-line tool nicer.".to_owned()),
            tags: vec!["rust".to_owned(), "cli".to_owned()],
            tasks: vec![
                Task {
                    title: "Document database flag".to_owned(),
                    completed: true,
                    priority: 2,
                },
                Task {
                    title: "Add shell completion".to_owned(),
                    completed: false,
                    priority: 3,
                },
            ],
        },
        Project {
            id: QuokkaId::new(),
            name: "Weekend projects".to_owned(),
            description: Some("A few low-pressure personal experiments.".to_owned()),
            tags: vec!["personal".to_owned()],
            tasks: vec![
                Task {
                    title: "Try a new QuokkaDB example".to_owned(),
                    completed: false,
                    priority: 1,
                },
                Task {
                    title: "Clean up notes".to_owned(),
                    completed: true,
                    priority: 3,
                },
            ],
        },
    ]
}
