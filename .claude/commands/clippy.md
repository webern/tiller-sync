# Changelog Command

Runs cargo fmt and cargo clippy and fixes issues.

## Usage

This command fixes up Rust code.

### `/clippy`

When running this command you should:

- First run `cargo fmt` at the root of the Rust project.
- Then run `cargo clippy --all-features -- -D warnings` and fix all problems.

Lastly, you should run `cargo test`. At this point if there are failing tests or compilation
problems you should ask the user what to do.
