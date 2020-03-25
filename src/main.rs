use std::env;
use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::*;
use serde_json::{json, Value};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    query_watchman(&args).with_context(|| {
        anyhow!("{:?} failed", args)
    })
}

fn query_watchman(args: &[String]) -> Result<()> {
    let git_work_tree = env::current_dir().context("Couldn't get working directory")?;

    let mut watchman = Command::new("watchman")
        .args(&["-j", "--no-pretty"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("Couldn't start watchman")?;

    {
        let time = &args[2];
        let time_nanoseconds: u64 = time
            .parse::<u64>()
            .context("Second arg wasn't an integer")?;
        let time_seconds = time_nanoseconds / 1_000_000_000;

        let watchman_query = json!(
            [
                "query",
                git_work_tree,
                {
                    "since": time_seconds,
                    "fields": ["name"],
                    "expression": [
                        "not", [
                            "allof",[
                                "since",
                                time_seconds,
                                "cclock"
                            ],
                            [
                                "not",
                                "exists"
                            ]
                        ]
                    ]
                }
            ]
        );

        watchman
            .stdin
            .as_mut()
            .expect("child Watchman process's stdin isn't piped")
            .write_all(watchman_query.to_string().as_bytes())?;
    }

    let output = watchman
        .wait_with_output()
        .context("Failed to wait on watchman query")?
        .stdout;

    let response: Value = serde_json::from_str(
        String::from_utf8(output)
            .context("Watchman didn't return valid JSON")?
            .as_str(),
    )?;

    if let Some(err) = response["error"].as_str() {
        ensure!(
            err.contains("unable to resolve root"),
            "Watchman failed for an unexpected reason {}",
            err
        );
        return add_to_watchman(&git_work_tree);
    }

    match response["files"].as_array() {
        Some(files) => {
            for file in files {
                if let Some(filename) = file.as_str() {
                    print!("{}\0", filename);
                }
            }

            Ok(())
        }
        None => bail!("missing file data"),
    }
}

fn add_to_watchman(worktree: &std::path::Path) -> Result<()> {
    eprintln!("Adding {} to Watchman's watch list", worktree.display());

    let watchman = Command::new("watchman")
        .args(&[
            "watch",
            worktree
                .to_str()
                .expect("Working directory isn't valid Unicode"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("Couldn't start watchman watch")?;

    let output = watchman
        .wait_with_output()
        .context("Failed to wait on `watchman watch`")?;
    ensure!(output.status.success(), "`watchman watch` failed");

    // Return the fast "everything is dirty" indication to Git.
    // This makes subsequent queries much faster since Git will pass Watchman
    // a timestamp from _after_ it started.
    // (When Watchman gets a time before its run,
    // it conservatively says everything has changed.)
    print!("/\0");
    Ok(())
}
