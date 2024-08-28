# Sqlite Merge Tool

This tool is used to merge the databases of some sqlite files. It is useful when you have two databases with the same schema and you want to merge them into one.

## Installation

```bash
cargo install smt --git https://github.com/WenChunTech/Sqlite-Merge-Tool.git
```

## Usage

```bash
smt -h
Usage: smt.exe --src <SRC> --dst <DST>

Options:
  -s, --src <SRC>  The source database file path, support glob pattern
  -d, --dst <DST>  The destination database file path
  -h, --help       Print help (see more with '--help')
  -V, --version    Print version

```

## Features
- merge multiple databases
- merge tables with the same structure

## Limitations
- the tables must have the same structure
- the tables primary key must be integer
- the tables must not have foreign keys
- the tables exist unique index
