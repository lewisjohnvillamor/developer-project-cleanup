# Security policy

Project Hibernate deletes directories. A bug that removes the wrong one is a
security issue and is treated as the highest priority.

## Reporting

Please **do not** open a public issue for anything that could cause data loss
or lets a crafted project folder make the app touch files outside it.

Use GitHub's private reporting: **Security → Report a vulnerability** on the
repository, which opens a private advisory only maintainers can see.
If that is unavailable, email the maintainer listed on the GitHub profile with
`[hibernate security]` in the subject.

Include: platform, version (Settings → About), how to reproduce, and what was
removed. You will get an acknowledgement within 72 hours.

## What counts

- Any path being removed that was not listed in the review dialog.
- Any way to make `validate_artifact` accept a path outside the project, a
  symlink/junction target, a protected name, or a filesystem root.
- Following symlinks or junctions during removal.
- Quarantine restore writing outside the original location.
- Wake running a command other than the one shown.
- Dependency vulnerabilities in anything that ships in the binaries.

## Supported versions

Only the latest release receives fixes.

## Design notes for reviewers

The safety gate is `crates/hibernate-core/src/cleanup/safety.rs`; its tests
enumerate the refusals. Removal never follows symlinks (`cleanup/remove.rs`),
trees are removed by their canonical path only after the gate, and rules match
directory names only, never files.
