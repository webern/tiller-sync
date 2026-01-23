# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [v0.2.0]

### Changed

- Dates are now stored in a useful (ISO) format in the database. [#28] fixed in [#29]

## [v0.1.1]

### Fixed

- Category, Hide From Reports is now persisted. [#24] fixed in [#26]

### Changed

- The secrets file is now copied instead of moved during `tiller init` [#23]

## [v0.1.0]

### Added

- Initial project structure, documentation and design [#3], [#4]
- Implement `tiller init` [#9]
- Implement OAuth [#5], [#6], [#7], [#8], [#10]
- Implement Sheets interactions [#11], [#13]
- Implement the Database Layer [#14], [#15], [#16]
- Implement `tiller sync down` and `tiller sync up`, mostly in [#17]
- Implement an MCP server for the `sync up` and `sync down` commands [#18]
- Implement crud [#19], [#21]
- Implement queries [#22]

<!-- @formatter:off -->

<!-- Tags -->

[Unreleased]: https://github.com/webern/tiller-sync/compare/v0.2.0...HEAD
[v0.2.0]: https://github.com/webern/tiller-sync/releases/tag/v0.2.0
[v0.1.1]: https://github.com/webern/tiller-sync/releases/tag/v0.1.1
[v0.1.0]: https://github.com/webern/tiller-sync/releases/tag/v0.1.0

<!-- Pull Requests -->
[#3]: https://github.com/webern/tiller-sync/pull/3
[#4]: https://github.com/webern/tiller-sync/pull/4
[#5]: https://github.com/webern/tiller-sync/pull/5
[#6]: https://github.com/webern/tiller-sync/pull/6
[#7]: https://github.com/webern/tiller-sync/pull/7
[#8]: https://github.com/webern/tiller-sync/pull/8
[#9]: https://github.com/webern/tiller-sync/pull/9
[#10]: https://github.com/webern/tiller-sync/pull/10
[#11]: https://github.com/webern/tiller-sync/pull/11
[#13]: https://github.com/webern/tiller-sync/pull/13
[#14]: https://github.com/webern/tiller-sync/pull/14
[#15]: https://github.com/webern/tiller-sync/pull/15
[#16]: https://github.com/webern/tiller-sync/pull/16
[#17]: https://github.com/webern/tiller-sync/pull/17
[#18]: https://github.com/webern/tiller-sync/pull/18
[#19]: https://github.com/webern/tiller-sync/pull/19
[#21]: https://github.com/webern/tiller-sync/pull/21
[#22]: https://github.com/webern/tiller-sync/pull/22
[#23]: https://github.com/webern/tiller-sync/pull/23
[#26]: https://github.com/webern/tiller-sync/pull/26
[#29]: https://github.com/webern/tiller-sync/pull/29

<!-- Issues -->
[#24]: https://github.com/webern/tiller-sync/issues/24
[#28]: https://github.com/webern/tiller-sync/issues/28
