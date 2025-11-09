# Changelog Command

Manage the CHANGELOG.md file following the Keep a Changelog specification. See
@../../docs/ai/CHANGELOG_INSTRUCTIONS.md for complete instructions.

## Usage

This command supports two modes:

### `/changelog pr`

Updates the [Unreleased] section of CHANGELOG.md when preparing to merge work to main (directly or
via pull request).

**What this command does:**

1. Analyzes git commits and changes since the branch diverged from main
2. Presents detected changes to you in a summary format
3. Prompts you to confirm, edit, or provide descriptions of the changes
4. Adds entries to the [Unreleased] section under appropriate categories (Added, Changed, Fixed,
   etc.)
5. Creates CHANGELOG.md if it doesn't exist

**Example workflow:**

```bash
# On your feature branch, ready to merge
/changelog pr

# Claude will show you detected changes:
# - 3 commits since branching from main
# - Modified files: src/sync.rs, src/cli.rs
# - Suggested categories based on commit messages
#
# You review and confirm/edit the categorization
# Claude updates CHANGELOG.md with your changes under [Unreleased]
```

### `/changelog release vX.Y.Z`

Prepares CHANGELOG.md and Cargo.toml for a tagged release. Run this when you're ready to create a
version tag.

**What this command does:**

1. Fetches tags from GitHub to validate version sequence
2. Verifies the provided version is the correct next semantic version
3. Updates Cargo.toml to match the release version (or confirms it already matches)
4. Runs `cargo build` and `cargo test` to ensure stability
5. Converts the [Unreleased] section to a dated, versioned release
6. Creates a new empty [Unreleased] section
7. Updates version comparison links at the bottom of CHANGELOG.md

**Example workflow:**

```bash
# On main branch, ready to tag v0.1.1
/changelog release v0.1.1

# Claude will:
# - Verify v0.1.1 is the next version after v0.1.0
# - Update Cargo.toml version to 0.1.1
# - Run cargo build and cargo test
# - Update CHANGELOG.md with the release
#
# Then you manually:
# - Review the changes
# - git add CHANGELOG.md Cargo.toml
# - git commit -m "chore: prepare release v0.1.1"
# - git tag v0.1.1
# - git push && git push --tags
```

## Instructions for Claude

When this command is invoked, follow the comprehensive instructions at
@../../docs/ai/CHANGELOG_INSTRUCTIONS.md.

### For `/changelog pr`:

1. Determine the branch point from main using git commands
2. Get commits since branch point: `git log main..HEAD --oneline`
3. Get modified files: `git diff main...HEAD --name-status`
4. Present a hybrid summary to the user:
    - List commits with messages
    - List modified files
    - Suggest categorization based on keywords in commits
5. Use the AskUserQuestion tool to confirm/edit the categorization and descriptions
6. Update or create CHANGELOG.md following the Keep a Changelog format
7. Add entries to [Unreleased] section under appropriate categories

### For `/changelog release vX.Y.Z`:

1. Extract version from command (e.g., "v0.1.1")
2. Fetch tags: `git fetch --tags`
3. List and parse tags: `git tag -l`
4. Validate version sequence:
    - Parse semantic versions
    - Verify this is the next logical version
    - Error if tag already exists
5. Read and update Cargo.toml:
    - Parse current version
    - Update to match release (without 'v' prefix)
    - Report if already matches
6. Run build and tests:
    - `cargo build` - must succeed
    - `cargo test` - must succeed
    - Stop if either fails
7. Update CHANGELOG.md:
    - Read current content
    - Transform [Unreleased] to versioned section
    - Add current date in ISO 8601 format (YYYY-MM-DD)
    - Create new empty [Unreleased] section
    - Update comparison links at bottom
8. Report completion and remind user of manual steps:
    - Review changes
    - Commit with message: `chore: prepare release vX.Y.Z`
    - Create tag: `git tag vX.Y.Z`
    - Push: `git push && git push --tags`

## Important Notes

- Always follow Keep a Changelog format and conventions
- Use ISO 8601 dates (YYYY-MM-DD)
- Maintain semantic versioning
- Keep entries in [Unreleased] until a release is tagged
- Categories: Added, Changed, Deprecated, Removed, Fixed, Security
- Write entries in imperative mood: "Add feature" not "Added feature"
- Be specific and user-focused in descriptions
- Mark breaking changes explicitly with **BREAKING:** prefix
