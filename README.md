# Sqlite Merge Tool

This tool is used to merge the databases of some sqlite files. It is useful when you have two databases with the same schema and you want to merge them into one.

## Installation

```bash
cargo install smt
```

## Usage

```bash
smt <database1> <database2> <output>
```

## Features
- merge multiple databases
- merge tables with the same structure

## Limitations
- the tables must have the same structure
- the tables primary key must be integer
- the tables must not have foreign keys
- the tables exist unique index
