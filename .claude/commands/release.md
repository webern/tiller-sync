# /release

Prepares the changes necessary to prior to release (i.e. prior to pushing crates.io and prior to
creating a version tag).

## Usage

Usage is `/release <version>` where `<version>` is a semantic version with or without the leading
`v`. For example `0.2.3` and `v0.2.3` both mean version `0.2.3`.

Examples:

- `/release 1.0.1` : means "prepare the changes to release version 1.0.1"
- `/release v0.4.5` : means "prepare the changes to release version 0.4.5"

## Instructions

- Update CHANGELOG.md
    - The "Unreleased" section will become the `<version>` section.
    - Create a new, empty "Unreleased" section above it.
    - Update the links section accordingly following the pattern you see there. In general, tags are
      lined to the GitHub page for the tag (note that our new version will not exist yet on GitHub),
      and the "Unreleased" section should show the diff between `<version>..HEAD`. Note that we will
      ALWAYS include the preceding `v` in our version tags: e.g. `v0.2.3`.
- Update Cargo.toml
    - Change the version in Cargo.toml
    - Run the following commands to check everything:
        - `cargo test`
        - `cargo publish --dry-run --allow-dirty`
