use std::env;
use std::io::Write;
use std::process::{exit, Command, Stdio};

use failure::*;
use serde_json::{json, Value};

fn main() {
    query_watchman().unwrap_or_else(|e| {
        eprintln!("{}", pretty_error(&e));
        exit(1);
    })
}

fn query_watchman() -> Fallible<()> {
    let git_work_tree = env::current_dir().context("Couldn't get working directory")?;

    let mut watchman = Command::new("watchman")
        .args(&["-j", "--no-pretty"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn().context("Couldn't start watchman")?;

    {
        let args: Vec<String> = env::args().collect();
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

    let output = watchman.wait_with_output()?.stdout;

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

fn add_to_watchman(worktree: &std::path::Path) -> Fallible<()> {
    let watchman = Command::new("watchman")
        .args(&[
            "watch",
            worktree
                .to_str()
                .expect("Working directory isn't valid Unicode"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn().context("Couldn't start watchman watch")?;

    let output = watchman.wait_with_output()?;
    ensure!(output.status.success(), "`watchman watch` failed");

    // Return the fast "everything is dirty" indication to Git.
    // This makes subsequent queries much faster since Git will pass Watchman
    // a timestamp from _after_ it started.
    // (When Watchman gets a time before its run,
    // it conservatively says everything has changed.)
    print!("/\0");
    Ok(())
}

// Borrowed lovingly from Burntsushi:
// https://www.reddit.com/r/rust/comments/8fecqy/can_someone_show_an_example_of_failure_crate_usage/dy2u9q6/
// Chains errors into a big string.
fn pretty_error(err: &failure::Error) -> String {
    let mut pretty = err.to_string();
    let mut prev = err.as_fail();
    while let Some(next) = prev.cause() {
        pretty.push_str(":\n");
        pretty.push_str(&next.to_string());
        if let Some(bt) = next.backtrace() {
            let mut bts = bt.to_string();
            // If RUST_BACKTRACE is not set, next.backtrace() gives us
            // Some(bt), but bt.to_string() gives us an empty string.
            // If we push a newline to the return value and nothing else,
            // we get something like:
            // ```
            // Some errror
            // :
            // Its cause
            // ```
            if !bts.is_empty() {
                bts.push_str("\n");
                pretty.push_str(&bts);
            }
        }
        prev = next;
    }
    pretty
}
