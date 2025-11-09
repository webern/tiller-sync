# Changelog Management Instructions

This document provides comprehensive instructions for managing the CHANGELOG.md file in this
project, following the Keep a Changelog specification.

## Quick Reference

- **For PR/Merge to main:** Use `/changelog pr` to update the [Unreleased] section
- **For tagged release:** Use `/changelog release vX.Y.Z` to prepare a versioned release

## Keep a Changelog Specification (v1.1.0)

The following is the complete Keep a Changelog specification that this project follows.

### What is a Changelog?

A changelog is a file which contains a curated, chronologically ordered list of notable changes
for each version of a project.

### Why Keep a Changelog?

To make it easier for users and contributors to see precisely what notable changes have been made
between each release (or version) of the project.

### Who Needs a Changelog?

People do. Whether consumers or developers, the end users of software are human beings who care
about what's in the software. When the software changes, people want to know why and how.

### How Do I Make a Good Changelog?

#### Guiding Principles

- Changelogs are for humans, not machines
- There should be an entry for every single version
- The same types of changes should be grouped
- Versions and sections should be linkable
- The latest version comes first
- The release date of each version is displayed
- Mention whether you follow Semantic Versioning

#### Types of Changes

Changes should be grouped to describe their impact on the project. Use these standard categories:

- **Added** for new features
- **Changed** for changes in existing functionality
- **Deprecated** for soon-to-be removed features
- **Removed** for now removed features
- **Fixed** for any bug fixes
- **Security** in case of vulnerabilities

#### Best Practices

1. **Use a consistent format:** Follow the structure shown in the example below
2. **Include all notable changes:** Don't rely on commit logs alone
3. **Use ISO 8601 date format:** YYYY-MM-DD (e.g., 2024-03-15)
4. **Maintain an Unreleased section:** Track upcoming changes before they're released
5. **Link to version comparisons:** Help users see diffs between versions
6. **Be explicit about breaking changes:** Highlight deprecations and removals
7. **Write for humans:** Use clear, descriptive language

#### What NOT to Do

- Don't dump commit logs as changelogs
- Don't ignore deprecations
- Don't use confusing date formats
- Don't omit important context

### Standard Changelog Format

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- New feature description

### Changed

- Change description

### Deprecated

- Deprecation notice

### Removed

- Removal description

### Fixed

- Bug fix description

### Security

- Security fix description

## [1.0.0] - 2024-03-15

### Added

- Initial release with core functionality

[Unreleased]: https://github.com/username/project/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/username/project/releases/tag/v1.0.0
```

## Project-Specific Versioning Guidelines

This project uses Semantic Versioning with git tags pushed to GitHub. All tags include the 'v'
prefix.

### Version Progression

1. **Early development (current):** `v0.0.1`, `v0.0.2`, `v0.0.3`, etc.
    - Incremental development
    - Frequent breaking changes acceptable
    - Focus on building core functionality

2. **Usable product:** `v0.1.0`, `v0.2.0`, etc.
    - Transition when the product becomes usable
    - Minor version bumps for new features
    - Patch versions for bug fixes

3. **Stable release (future):** `v1.0.0` and beyond
    - Indicates production readiness
    - Semantic versioning strictly followed
    - Breaking changes require major version bump

### Semantic Versioning Rules

Given a version number MAJOR.MINOR.PATCH (e.g., v1.2.3):

- **MAJOR:** Incompatible API changes
- **MINOR:** Backward-compatible functionality additions
- **PATCH:** Backward-compatible bug fixes

Pre-1.0.0 versions (v0.y.z) are for initial development where anything may change.

## Workflow for `/changelog pr`

When preparing a branch for merge to main (directly or via PR), use `/changelog pr` to update the
changelog. This command should:

### 1. Analyze Changes

- Get git commits since the last merge to main (or branch point)
- Examine modified files and their changes
- Present a summary of detected changes to the user

### 2. Categorize Changes (Hybrid Approach)

- Show the user the detected commits and changes
- Suggest appropriate categories (Added, Changed, Fixed, etc.)
- Prompt the user to confirm, edit, or provide their own description
- Allow the user to reorganize entries into appropriate categories

### 3. Update CHANGELOG.md

- Add entries to the `[Unreleased]` section under appropriate category headings
- Maintain alphabetical or logical ordering within categories
- Use present tense for consistency (e.g., "Add feature" not "Added feature")
- Keep descriptions concise but informative
- Create CHANGELOG.md if it doesn't exist (using standard format)

### 4. Example [Unreleased] Section After PR

```markdown
## [Unreleased]

### Added

- CLI argument parsing with clap for subcommands
- Support for syncing Tiller sheets to SQLite database

### Fixed

- Incorrect date formatting in sync operations
```

## Workflow for `/changelog release vX.Y.Z`

When preparing a tagged release, use `/changelog release v0.1.1` (or appropriate version). This
command should:

### 1. Fetch and Validate Tags

- Run `git fetch --tags` to get latest tags from GitHub
- Parse all existing tags to determine the latest version
- Verify that the provided version is the correct sequential next version
- Warn if the version doesn't follow semantic versioning rules
- Error if the tag already exists

### 2. Update Cargo.toml Version

- Parse `Cargo.toml` to get current version
- Check if version matches the release tag (without 'v' prefix)
- Update `Cargo.toml` version if needed
- Report if version already matches

### 3. Run Build and Tests

- Execute `cargo build` to ensure the project builds
- Execute `cargo test` to ensure all tests pass
- Stop the release process if either command fails
- Provide clear error messages about what failed

### 4. Update CHANGELOG.md

- Convert the `[Unreleased]` section to a versioned release section
- Add the release date in ISO 8601 format (YYYY-MM-DD)
- Create a new empty `[Unreleased]` section above it
- Update comparison links at the bottom of the file
- Ensure proper formatting according to Keep a Changelog

### 5. Example Transformation

**Before `/changelog release v0.1.1`:**

```markdown
## [Unreleased]

### Added

- New sync feature for transactions

### Fixed

- Bug in date parsing

[Unreleased]: https://github.com/webern/tiller/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/webern/tiller/releases/tag/v0.1.0
```

**After `/changelog release v0.1.1`:**

```markdown
## [Unreleased]

## [0.1.1] - 2025-11-09

### Added

- New sync feature for transactions

### Fixed

- Bug in date parsing

[Unreleased]: https://github.com/webern/tiller/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/webern/tiller/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/webern/tiller/releases/tag/v0.1.0
```

### 6. Final State

After the command completes successfully:

- CHANGELOG.md is updated with the new version
- Cargo.toml version matches the tag (without 'v')
- Project builds and tests pass
- Files are ready for manual review, commit, and tagging

The user will then:

1. Review the changes
2. Commit with a message like: `chore: prepare release v0.1.1`
3. Create the tag: `git tag v0.1.1`
4. Push the commit and tag: `git push && git push --tags`

## Writing Good Changelog Entries

### Be Specific and Contextual

**Good:**

- Add SQLite connection pooling to improve concurrent sync performance
- Fix off-by-one error in transaction date calculations

**Bad:**

- Add pooling
- Fix bug

### Use Consistent Voice

- Use imperative mood: "Add feature" not "Added feature" or "Adds feature"
- Start with a verb: Add, Fix, Change, Remove, Deprecate
- Be concise but complete

### Provide User Value

Focus on what changed for the user, not implementation details.

**Good:**

- Add support for syncing multiple Tiller sheets simultaneously

**Bad:**

- Refactor SyncService to use async/await instead of callbacks

### Group Related Changes

If a feature requires multiple changes, consider grouping them:

**Good:**

```markdown
### Added

- Complete OAuth2 authentication flow
    - Add Google OAuth2 client configuration
    - Implement token refresh mechanism
    - Add secure token storage in keychain
```

## Common Scenarios

### Breaking Changes

When introducing breaking changes, be explicit:

```markdown
### Changed

- **BREAKING:** Rename `sync` command to `pull` for clarity with new `push` command
```

### Security Fixes

Always use the Security section for vulnerabilities:

```markdown
### Security

- Fix SQL injection vulnerability in custom query filtering
```

### Deprecations

Give users advance warning:

```markdown
### Deprecated

- `--format json` flag is deprecated in favor of `--output json`, will be removed in v0.3.0
```

## Troubleshooting

### Version Mismatch Errors

If `Cargo.toml` version doesn't match the tag:

- The command will update `Cargo.toml` automatically
- Review the change before committing

### Tag Already Exists

If attempting to release a version that's already tagged:

- Check `git tag -l` to see existing tags
- Use the next sequential version
- Consider if you meant to create a patch release

### Tests Fail During Release

If `cargo test` fails:

- Fix the failing tests first
- Don't proceed with the release until tests pass
- Update changelog to note the fixes

## Reference Links

- [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
- [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
- [ISO 8601 Date Format](https://www.iso.org/iso-8601-date-and-time-format.html)
