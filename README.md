# rs-git-fsmonitor

A git fsmonitor hook written in Rust.

## Installation

Install the tool:

`brew tap jgavris/rs-git-fsmonitor git@github.com:jgavris/rs-git-fsmonitor.git && brew install rs-git-fsmonitor`

Configure git repository to use the tool (run in desired large git repository):

`git config core.fsmonitor rs-git-fsmonitor`

## Purpose

Git 2.16 added support for a `core.fsmonitor` hook to allow an external tool to inform it which files have changed.

https://blog.github.com/2018-04-05-git-217-released/#speeding-up-status-with-watchman

On repositories with many files, this can be a dramatic speedup.

```shell
λ find . -type f | wc -l
   30737
```

Before:

```shell
λ time git status
On branch master
Your branch is up to date with 'origin/master'.

nothing to commit (use -u to show untracked files)

real	0m0.129s
user	0m0.062s
sys	0m0.268s
```

After:

```shell
λ time git status
On branch master
Your branch is up to date with 'origin/master'.

nothing to commit (use -u to show untracked files)

real	0m0.067s
user	0m0.030s
sys	0m0.026s
```

## License

MIT
