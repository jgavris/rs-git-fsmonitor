use std::env;
use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, bail, ensure, Context as _, Result};
use serde_json::{json, Value};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        bail!("Expected two arguments, got {:?}", &args[1..]);
    }
    let hook_version = args[1]
        .parse::<isize>()
        .context("First arg wasn't a version number")?;

    match hook_version {
        1 => query_watchman_v1(&args),
        2 => query_watchman_v2(&args),
        _ => bail!(
            "Unsupported fsmonitor-watchman hook version: {}",
            hook_version
        ),
    }
}

/// V2 of the API takes a clock token from `watchman clock`,
/// and asks watchman what files have changed since the provided time.
fn query_watchman_v2(args: &[String]) -> Result<()> {
    let worktree = env::current_dir().context("Couldn't get working directory")?;

    let last_update_token = &args[2];

    // Gracefully upgrade repo fsmonitor from v1 timestmap to v2 opaque clock token.
    let token_value = if last_update_token.starts_with('c') {
        Value::from(last_update_token.to_string())
    } else {
        Value::from(last_update_token.parse::<u64>().unwrap_or(0) / 1_000_000_000)
    };

    // From the Perl that ships with Git:
    //
    // In the query expression below we're asking for names of files that
    // changed since $last_update_token but not from the .git folder.
    //
    // To accomplish this, we're using the "since" generator to use the
    // recency index to select candidate nodes and "fields" to limit the
    // output to file names only. Then we're using the "expression" term to
    // further constrain the results.
    let response = watchman_query(&json!(
        [
            "query",
            worktree,
            {
                "since": token_value,
                "fields": ["name"],
                "expression": [
                    "not", [
                        "dirname",
                        ".git"
                    ]
                ]
            }
        ]
    ))?;

    if let Some(err) = response["error"].as_str() {
        ensure!(
            err.contains("unable to resolve root") || err.contains("is not watched"),
            "Watchman failed for an unexpected reason {}",
            err
        );

        // Start a watch, then get the clock ID.
        add_watch(&worktree)?;
        let clock_id = watchman_clock(&worktree)?;

        // Return the fast "everything is dirty" indication to Git.
        // This makes subsequent queries much faster since Git will pass Watchman
        // a timestamp from _after_ it started.
        // (When Watchman gets a time before its run,
        // it conservatively says everything has changed.)
        print!("{clock_id}\0/\0");
        return Ok(());
    }

    let new_clock_id = response["clock"]
        .as_str()
        .ok_or_else(|| anyhow!("Missing new clock ID in watchman response {:#}", response))?;

    match response["files"].as_array() {
        Some(files) => {
            print!("{new_clock_id}\0");
            for file in files {
                if let Some(filename) = file.as_str() {
                    print!("{filename}\0");
                }
            }

            Ok(())
        }
        None => bail!("Missing file data in watchman response {:#}", response),
    }
}

/// Calls `watchman clock` on the Git directory and returns the provided ID.
fn watchman_clock(worktree: &Path) -> Result<String> {
    let watchman = Command::new("watchman")
        .args([OsStr::new("clock"), worktree.as_os_str()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("Couldn't start `watchman clock`")?;

    let output = watchman
        .wait_with_output()
        .context("Failed to wait on `watchman clock`")?
        .stdout;

    let response: Value = serde_json::from_str(std::str::from_utf8(&output)?)?;

    response["clock"].as_str().map_or_else(
        || {
            Err(anyhow!(
                "`watchman clock` didn't provide a clock ID in response {response:#}"
            ))
        },
        |clock_id| Ok(String::from(clock_id)),
    )
}

/// V1 of the API takes a time of elapsed nanoseconds since the POSIX epoch,
/// and asks watchman what files have changed since the provided time.
fn query_watchman_v1(args: &[String]) -> Result<()> {
    let worktree = env::current_dir().context("Couldn't get working directory")?;

    let time = &args[2];
    let time_nanoseconds: u64 = time
        .parse::<u64>()
        .context("Second arg wasn't an integer")?;
    let time_seconds = time_nanoseconds / 1_000_000_000;

    // From the Perl that ships with Git:
    //
    // In the query expression below we're asking for names of files that
    // changed since $time but were not transient (ie created after
    // $time but no longer exist).
    //
    // To accomplish this, we're using the "since" generator to use the
    // recency index to select candidate nodes and "fields" to limit the
    // output to file names only. Then we're using the "expression" term to
    // further constrain the results.
    //
    // The category of transient files that we want to ignore will have a
    // creation clock (cclock) newer than $time_t value and will also not
    // currently exist.
    let response = watchman_query(&json!(
        [
            "query",
            worktree,
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
    ))?;

    if let Some(err) = response["error"].as_str() {
        ensure!(
            err.contains("unable to resolve root") || err.contains("is not watched"),
            "Watchman failed for an unexpected reason {}",
            err
        );
        add_watch(&worktree)?;
        // Return the fast "everything is dirty" indication to Git.
        // This makes subsequent queries much faster since Git will pass Watchman
        // a timestamp from _after_ it started.
        // (When Watchman gets a time before its run,
        // it conservatively says everything has changed.)
        print!("/\0");
        return Ok(());
    }

    match response["files"].as_array() {
        Some(files) => {
            for file in files {
                if let Some(filename) = file.as_str() {
                    print!("{filename}\0");
                }
            }

            Ok(())
        }
        None => bail!("Missing file data in watchman response {:#}", response),
    }
}

fn watchman_query(query: &Value) -> Result<Value> {
    let mut watchman = Command::new("watchman")
        .args(["-j", "--no-pretty"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("Couldn't start watchman")?;

    watchman
        .stdin
        .as_mut()
        .expect("child Watchman process's stdin isn't piped")
        .write_all(query.to_string().as_bytes())?;

    let output = watchman
        .wait_with_output()
        .context("Failed to wait on watchman query")?
        .stdout;

    let as_json = serde_json::from_str(std::str::from_utf8(&output)?)?;
    Ok(as_json)
}

fn add_watch(worktree: &Path) -> Result<()> {
    eprintln!("Adding {} to Watchman's watch list", worktree.display());

    let watchman = Command::new("watchman")
        .args([OsStr::new("watch"), worktree.as_os_str()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("Couldn't start watchman watch")?;

    let output = watchman
        .wait_with_output()
        .context("Failed to wait on `watchman watch`")?;
    ensure!(output.status.success(), "`watchman watch` failed");

    Ok(())
}
