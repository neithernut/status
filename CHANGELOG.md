# Changelog

All notable changes to this project will be documented in this file. Its format
is based on https://keepachangelog.com/en/1.1.0/.

## Unreleased

### Added

- Support handling of files with uncertain paths in internal utility
  `ReadItemInstaller`.
- Add support for including memory usage information in the status.
- Introduce utility for moving averages for composition of data sources.
- Introduce internal utilities for autoscaling values, such as memory sizes.
- Introduce additional internal representations for entries for future use.
- Add tests for internal entry structures.

### Changed

- Change the mechanism through which labels are introduced into a status line.
- Refactor specification application: move parts to separate functions.
- Change the mechanism through which entries are formatted internally.
- Replace internal utility `Word` for extracting whitespace-delimited portions
  with the more general `Simple`, which allows for other delimiters.
- Add `mock_instant` as a direct development dependency for use in tests.
- Make internal data sources composable, avoiding code duplication.
- Create io-uring with flags that may reduce overall resource usage.
- Reorder specifiers in [README](./README.md) to reflect the order they appear
  in the code parsing them.


## 0.1.0 - 2024-06-01

### Added

- Initial (Rust) version of the `status` utility.
- README, including building and preliminary usage documentation.
