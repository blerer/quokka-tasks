use std::process::Command;
use tempfile::TempDir;

fn run(db: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_quokka-tasks"))
        .arg("--db")
        .arg(db)
        .args(args)
        .output()
        .expect("quokka-tasks should run");

    assert!(
        output.status.success(),
        "command failed: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("CLI output should be UTF-8")
}

fn run_failure(db: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_quokka-tasks"))
        .arg("--db")
        .arg(db)
        .args(args)
        .output()
        .expect("quokka-tasks should run");

    assert!(!output.status.success(), "command unexpectedly succeeded");
    String::from_utf8(output.stderr).expect("CLI error output should be UTF-8")
}

#[test]
fn data_survives_reopening_and_tag_filtering() {
    let directory = TempDir::new().unwrap();
    run(
        directory.path(),
        &[
            "project",
            "new",
            "QuokkaDB launch",
            "--description",
            "Release work",
            "--tag",
            "oss",
            "--tag",
            "release",
        ],
    );

    let output = run(directory.path(), &["list", "--tag", "oss"]);
    assert!(output.contains("QuokkaDB launch"));
    assert!(output.contains("tags: oss, release"));

    let output = run(directory.path(), &["list", "--tag", "missing"]);
    assert_eq!(output.trim(), "No projects found.");
}

#[test]
fn completing_one_task_preserves_its_siblings() {
    let directory = TempDir::new().unwrap();
    let db = directory.path();
    let name = "Sibling test";
    run(db, &["project", "new", name]);
    run(db, &["task", "add", name, "Review", "--priority", "1"]);
    run(db, &["task", "add", name, "Review", "--priority", "3"]);
    assert_eq!(
        run(db, &["task", "done", name, "1"]).trim(),
        "Completed “Review”."
    );
    assert!(run(db, &["task", "done", name, "1"]).contains("already complete"));
    assert_eq!(
        run(db, &["list"]).trim(),
        "Sibling test\n  1. [x] Review (high)\n  2. [ ] Review (low)"
    );
    assert_eq!(
        run(db, &["tasks", "--unfinished"]).trim(),
        "Sibling test\n  2. [ ] Review (low)"
    );
    run(db, &["task", "done", name, "2"]);
    assert!(run(db, &["tasks", "--unfinished"]).contains("No unfinished tasks"));
}

#[test]
fn reports_selector_errors() {
    let directory = TempDir::new().unwrap();
    let db = directory.path();
    run(db, &["project", "new", "Launch"]);
    assert!(run_failure(db, &["project", "new", "Launch"]).contains("already exists"));
    assert!(
        run_failure(db, &["task", "done", "launch", "1"])
            .contains("project “launch” was not found")
    );
    assert!(run_failure(db, &["task", "done", "Launch", "1"]).contains("has no tasks"));
    run(db, &["task", "add", "Launch", "Review"]);
    for number in ["0", "2", "-1", "abc"] {
        assert!(
            run_failure(db, &["task", "done", "Launch", number]).contains("1 to 1 for “Launch”")
        );
    }
}

#[test]
fn supports_legacy_tasks_and_ambiguous_project_names() {
    use quokkadb::{QuokkaDB, doc};
    let directory = TempDir::new().unwrap();
    let db = directory.path();
    {
        let database = QuokkaDB::open(db).unwrap();
        let projects = database.collection("projects").create_if_missing();
        for id in [41_i64, 42] {
            projects.insert_one(doc! {
                "_id": id, "name": "Legacy", "tags": [],
                "tasks": [{ "id": 99_i64, "title": "Original", "priority": 2, "completed": false }]
            }).unwrap();
        }
    }
    assert!(
        run_failure(db, &["task", "done", "Legacy", "1"])
            .contains("--project-id 41 or --project-id 42")
    );
    run(db, &["task", "add", "--project-id", "42", "New"]);
    run(db, &["task", "done", "--project-id", "42", "1"]);
    let listed = run(db, &["list"]);
    assert!(listed.contains("1. [ ] Original"));
    assert!(listed.contains("1. [x] Original\n  2. [ ] New"));
}

#[test]
fn unfinished_query_hides_completed_tasks_and_demo_is_idempotent() {
    let directory = TempDir::new().unwrap();
    let first = run(directory.path(), &["demo"]);
    assert_eq!(first.trim(), "Added 3 demo project(s).");

    let second = run(directory.path(), &["demo"]);
    assert_eq!(second.trim(), "Demo data is already present.");

    let unfinished = run(directory.path(), &["tasks", "--unfinished"]);
    assert!(unfinished.contains("Write announcement"));
    assert!(unfinished.contains("Add shell completion"));
    assert!(!unfinished.contains("Publish crate"));
    assert!(!unfinished.contains("Document database flag"));
}
