# COMMANDINDEX.md

## Purpose

This file defines stable project instructions for CommandIndex contributors and agents.

## Development Rules

- Use TDD by default.
- For every new behavior change:
  - add or update a failing test first
  - implement the smallest change to make it pass
  - refactor only after the test suite is green
- Do not merge untested behavior.
- Prefer integration tests for cross-module behavior.
- Prefer focused module tests for local state or parsing logic.

## Current Testing Policy

- `tests/cli_args.rs`
  - subcommand parsing
  - help and version flags
  - error handling for unknown subcommands

## Product Guardrails

- Index is always a regenerable derivative — never the source of truth.
- `clean` → `index` must always restore a working state.
- Search must return results within 500ms for 1500-file repositories.
- CLI output must support three formats: human, json, path.
- Japanese and English text must both be searchable.
