use std::env;
use std::fmt::Write;

use anyhow::{bail, ensure, Context as _, Result};
use watchman_client::prelude::*;
use watchman_client::Error as WatchmanError;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        bail!("Expected two arguments, got {:?}", &args[1..]);
    }
    let hook_version = args[1]
        .parse::<isize>()
        .context("First arg wasn't a version number")?;

    let client = Connector::new().connect().await?;
    let root = client
        .resolve_root(CanonicalPath::canonicalize(".")?)
        .await?;

    match hook_version {
        1 => query_watchman_v1(client, &root, &args).await,
        2 => query_watchman_v2(client, &root, &args).await,
        _ => bail!(
            "Unsupported fsmonitor-watchman hook version: {}",
            hook_version
        ),
    }
}

/// V2 of the API takes a clock token from `watchman clock`,
/// and asks watchman what files have changed since the provided time.
async fn query_watchman_v2(client: Client, root: &ResolvedRoot, args: &[String]) -> Result<()> {
    let last_update_token = &args[2];

    // Gracefully upgrade repo fsmonitor from v1 timestamp to v2 opaque clock token.
    let since = if last_update_token.starts_with('c') {
        ClockSpec::StringClock(last_update_token.to_owned())
    } else {
        ClockSpec::UnixTimestamp(last_update_token.parse::<i64>().unwrap_or(0) / 1_000_000_000)
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
    let result = client
        .query::<NameOnly>(
            root,
            QueryRequestCommon {
                since: Some(Clock::Spec(since)),
                expression: Some(Expr::Not(Box::new(Expr::DirName(DirNameTerm {
                    path: ".git".into(),
                    depth: None,
                })))),
                ..Default::default()
            },
        )
        .await;

    if let Ok(result) = result {
        let files = result
            .files
            .context("Missing file data in watchman response")?;

        let clock = match result.clock {
            Clock::Spec(ClockSpec::StringClock(string)) => Some(string),
            _ => None,
        }
        .unwrap_or_default();

        let output = files.iter().fold(format!("{clock}\0"), |mut acc, file| {
            if let Some(filename) = file.name.to_str() {
                write!(acc, "{filename}\0").unwrap();
            }
            acc
        });
        print!("{output}");
    } else {
        // Start a watch, then get the clock ID.
        let clock = match client.clock(root, SyncTimeout::Default).await? {
            ClockSpec::StringClock(string) => Some(string),
            ClockSpec::UnixTimestamp(_) => None,
        }
        .unwrap_or_default();

        // Return the fast "everything is dirty" indication to Git.
        // This makes subsequent queries much faster since Git will pass Watchman
        // a timestamp from _after_ it started.
        // (When Watchman gets a time before its run,
        // it conservatively says everything has changed.)
        print!("{clock}\0/\0");
    }

    Ok(())
}

/// V1 of the API takes a time of elapsed nanoseconds since the POSIX epoch,
/// and asks watchman what files have changed since the provided time.
async fn query_watchman_v1(client: Client, root: &ResolvedRoot, args: &[String]) -> Result<()> {
    let time = &args[2];
    let time_nanoseconds = time
        .parse::<i64>()
        .context("Second arg wasn't an integer")?;
    let timestamp = ClockSpec::UnixTimestamp(time_nanoseconds / 1_000_000_000);

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
    let result = client
        .query::<NameOnly>(
            root,
            QueryRequestCommon {
                since: Some(Clock::Spec(timestamp.clone())),
                expression: Some(Expr::Not(Box::new(Expr::All(vec![
                    Expr::Since(SinceTerm::CreatedClock(timestamp)),
                    Expr::Not(Box::new(Expr::Exists)),
                ])))),
                ..Default::default()
            },
        )
        .await;

    match result {
        Ok(result) => {
            let files = result
                .files
                .context("Missing file data in watchman response")?;

            let output = files.iter().fold(String::default(), |mut acc, file| {
                if let Some(filename) = file.name.to_str() {
                    write!(acc, "{filename}\0").unwrap();
                }
                acc
            });
            print!("{output}");

            Ok(())
        }
        Err(WatchmanError::WatchmanResponseError { message }) => {
            ensure!(
                message.contains("unable to resolve root") || message.contains("is not watched"),
                "Watchman failed for an unexpected reason {}",
                message
            );

            // Return the fast "everything is dirty" indication to Git.
            // This makes subsequent queries much faster since Git will pass Watchman
            // a timestamp from _after_ it started.
            // (When Watchman gets a time before its run,
            // it conservatively says everything has changed.)
            print!("/\0");

            Ok(())
        }
        Err(err) => bail!(err),
    }
}
