# brr
> A dead-simple, ultra-fast build farm for Rust

## Overview

Brr is the Buildrecall CLI. If you're not familiar with Buildrecall, [we make your builds run really fast](https://buildrecall.com/).

**Features:**
- 48 CPU cores, 192 GiB RAM
- Starts your build incrementally while you're programming.
- Use your existing CI, just replaces your build step.

## Install

```bash
cargo install brr
```

## Usage

Login to Buildrecall with a bash-history safe token [you can get here.](https://buildrecall.com/setup)

```bash
brr login <token>
```

Attach a build farm to a repository on your local development environment. 

```bash
# ./my-rust-project
brr attach my-rust-project
```

Create a job you'd like to run in the `buildrecall.toml` that was just created:

```toml
[project]
name = 'my-rust-project'

[[jobs]]
name = "mybuild"
run = "cargo build --release"
artifacts = ["target"]
```

Run your job:
```bash
brr run mybuild
```

In your CI (such as Github Actions), add a `BUILDRECALL_API_KEY` environment variable (you can get a key [here](https://buildrecall.com/setup)), and then you don't need to login:

```bash
BUILDRECALL_API_KEY=my_secret_key brr run mybuild
```
