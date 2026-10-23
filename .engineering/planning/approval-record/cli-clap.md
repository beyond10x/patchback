---
format: aep.planning-md/3
id: approval-record:cli-clap
kind: approval-record
status: draft
title: CLI parsing with clap derive, decided by b10x-chef-orchestrator
relations:
- decides: story:cli-clap
revision: 1
---
# CLI parsing with clap derive, decided by b10x-chef-orchestrator

## The open item

`approval-record:wave-a1` left one item open: the workspace rule asks for clap derive on command
lines, DESIGN.md § 3 (revision 0.3) admitted no CLI-parsing crate, and wave a1 shipped the CLI on
std-only parsing. This record closes that item.

## Who decided, in their words

Message from session `b10x-chef-orchestrator`, 2026-10-23, under the grant in the operator's
workspace instructions: "CLI decision: clap. The workspace rule in ~/beyond10x/AGENTS.md, headed
"Running code is Rust (Timo, 2026-09-27, hard rule)", requires clap derive for command lines.
DESIGN.md § 3 forbids ML and numeric crates, and line 16 says CI "rejects unreviewed additions",
which means reviewed additions are allowed. So admit `clap` with the derive feature as the single
reviewed CLI dependency: Amend DESIGN.md § 3 with one row, "CLI parsing: clap (derive), required by
the workspace rule". Add clap to the Cargo.lock gate's allow list. Port the CLI to clap derive. Do
this as the first unit of the next wave."

## The decision

- `clap` with `derive` is the one reviewed CLI-parsing crate of the application.
- It lands as `story:cli-clap`, the first unit of wave a2, before the other units fork.
- The std-only parsing of wave a1 is replaced.
