use std::env;
use std::io::{self, Error, ErrorKind, Write};
use std::process::{exit, Command, Stdio};

extern crate serde;

#[macro_use]
extern crate serde_json;

use serde_json::Value;

fn main() {
    query_watchman().unwrap_or_else(|e| {
        eprintln!("{}", e);
        exit(1);
    })
}

fn query_watchman() -> io::Result<()> {
    let git_work_tree = env::current_dir().unwrap();

    let mut watchman = Command::new("watchman")
        .args(&["-j", "--no-pretty"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    {
        let args: Vec<String> = env::args().collect();
        let time = &args[2];
        let time_nanoseconds: u64 = time.parse::<u64>().unwrap();
        let time_seconds = time_nanoseconds / 1000000000;

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
            .unwrap()
            .write_all(watchman_query.to_string().as_bytes())?;
    }

    let output = watchman.wait_with_output()?.stdout;

    let response: Value = serde_json::from_str(String::from_utf8(output).unwrap().as_str())?;

    match response["error"].as_str() {
        Some(_) => return add_to_watchman(git_work_tree),
        None => {}
    }

    match response["files"].as_array() {
        Some(files) => {
            for file in files {
                match file.as_str() {
                    Some(filename) => print!("{}\0", filename),
                    None => {}
                }
            }

            return Ok(());
        }
        None => return Err(Error::new(ErrorKind::Other, "missing file data")),
    }
}

fn add_to_watchman(worktree: std::path::PathBuf) -> io::Result<()> {
    let watchman = Command::new("watchman")
        .args(&["watch", worktree.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    match watchman.wait_with_output() {
        Ok(_) => {
            print!("\0");
            return Ok(());
        }
        Err(e) => return Err(e),
    }
}
