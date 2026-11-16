# Quokka Tasks

Quokka Tasks is a local project tracker. Each project is one QuokkaDB document
with a checklist of embedded tasks.

Powered by [QuokkaDB](https://quokkadb.io). Learn more on the
[QuokkaDB GitHub repository](https://github.com/blerer/quokkadb).

## Try it from the checkout

Start with a fresh `./showcase-data` directory. The first command creates the
database and seeds three projects. The second lists their unfinished tasks.

```sh
cargo run --locked -- --db ./showcase-data demo
cargo run --locked -- --db ./showcase-data tasks --unfinished
```

“QuokkaDB launch” shows one unfinished task, “Write announcement”. QuokkaDB
selects projects with `project.tasks.any(|task| task.completed.eq(false))`.
The CLI then skips completed tasks when printing each matching project.

```sh
cargo run --locked -- --db ./showcase-data task done "QuokkaDB launch" 1
cargo run --locked -- --db ./showcase-data tasks --unfinished
```

The completion command converts checklist number 1 to index 0 and writes
`p.tasks.at(index).completed.set(true)`. “QuokkaDB launch” disappears from
the final listing because its last unfinished task is now complete. Running
`demo` again preserves those edits and skips projects whose names already exist.

## Install and run

```sh
cargo install --locked --path .
quokka-tasks --help
```

## A short session

The installed CLI uses `./data/quokka-tasks` by default and creates it on first use.
Start in a directory with a fresh default database for this session.
Use the exact project name and the task number shown in its checklist.

```text
$ quokka-tasks project new "QuokkaDB launch" --description "Prepare the open-source release" --tag oss
Created project “QuokkaDB launch”.

$ quokka-tasks task add "QuokkaDB launch" "Draft release notes" --priority 1
Added “Draft release notes” to “QuokkaDB launch”.

$ quokka-tasks task add "QuokkaDB launch" "Write announcement"
Added “Write announcement” to “QuokkaDB launch”.

$ quokka-tasks list --tag oss
QuokkaDB launch
  Prepare the open-source release
  tags: oss
  1. [ ] Draft release notes (high)
  2. [ ] Write announcement

$ quokka-tasks task done "QuokkaDB launch" 1
Completed “Draft release notes”.

$ quokka-tasks tasks --unfinished
QuokkaDB launch
  Prepare the open-source release
  tags: oss
  2. [ ] Write announcement
```

Task numbers follow creation order within each project and stay the same in
filtered output. Project names are case-sensitive.

## Priority convention

Priorities are intentionally simple: `1` is highest, `2` is normal and the
default, and `3` is lowest. Values outside `1` through `3` are rejected.

## The Rust model

The root model derives `QuokkaDocument` and maps its ID to `_id`. The embedded
task derives `QuokkaType`; it does not need a collection identity of its own.

```rust
use quokkadb::{QuokkaDocument, QuokkaId, QuokkaType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, QuokkaType)]
pub struct Task {
    pub title: String,
    pub completed: bool,
    pub priority: u8,
}

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
```

## Persistence operations

Open the local database and select its typed project collection.

```rust
let db = QuokkaDB::open(path)?;
let projects = db
    .typed_collection::<Project>("projects")
    .create_if_missing();
```

For `task add`, the CLI reads the project to resolve its name or ID. The store
then appends the new task with a typed `push` that updates only the array.

```rust
let result = projects.update_one(
    |project| project.id.eq(project_id),
    |project| project.tasks.push(task.clone()),
)?;
```

Tag filtering and unfinished-project selection use generated typed fields.

```rust
let tagged = projects
    .find(|project| project.tags.any_eq(tag))
    .execute_collect()?;

let with_unfinished_tasks = projects
    .find(|project| project.tasks.any(|task| task.completed.eq(false)))
    .execute_collect()?;
```

Completion checks the checklist number against the stored tasks and updates
the selected task at its zero-based array index.

```rust
projects.update_one(
    |p| p.id.eq(project.id),
    |p| p.tasks.at(index).completed.set(true),
)?;
```

## Existing databases

If older projects share a
name, the CLI lists the selectors you can use, such as
`quokka-tasks task done --project-id 123 2`.

Scripts using the previous positional IDs should pass the project name (or
`--project-id`) and the task's checklist number. Existing databases open
directly, including tasks with legacy IDs.
