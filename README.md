# brr
> A dead-simple, ultra-fast build farm

## Overview

Brr is the Buildrecall CLI. If you're not familiar with Buildrecall, [we make your builds run really fast](https://buildrecall.com/).

**Features:**
- Use up to 96 CPU cores.
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

Attach a build farm to a repository on yo ur local development environment. 

```bash
# ./my-rust-project
brr attach
```

In your CI (such as Github Actions), add a `BUILDRECALL_API_KEY` environment variable (you can get a key here https://buildrecall.com), and then replace your build step with `brr pull`:
```bash
# This replaces your 'cargo build' step in CI
brr pull
```
