use quokkadb::{QuokkaDocument, QuokkaId, QuokkaType};
use serde::{Deserialize, Serialize};

/// A task embedded in a project document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, QuokkaType)]
pub struct Task {
    pub title: String,
    pub completed: bool,
    pub priority: u8,
}

/// A personal project and its embedded tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, QuokkaDocument)]
pub struct Project {
    #[serde(rename = "_id")]
    pub id: QuokkaId,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub tasks: Vec<Task>,
}
