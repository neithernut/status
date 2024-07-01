# Changelog

All notable changes to this project will be documented in this file. Its format
is based on https://keepachangelog.com/en/1.1.0/.

## Unreleased

### Added

- Added mechanisms for processing some values at a lower rate.
- Added support for including information related to batteries in the status.
- Supported handling of files with uncertain paths in internal utility
  `ReadItemInstaller`.
- Added support for including memory usage information in the status.
- Introduced utility for moving averages for composition of data sources.
- Introduced internal utilities for autoscaling values, such as memory sizes.
- Introduced additional internal representations for entries for future use.
- Added tests for internal entry structures.

### Changed

- Made values not update every tick in order to reduce the amount of CPU time
  consumed.
- Registered file descriptors we read from via the IO uring. This could decrease
  resource consumption.
- Changed the mechanism through which labels are introduced into a status line.
- Refactored specification application: moved parts to separate functions.
- Changed the mechanism through which entries are formatted internally.
- Replaced internal utility `Word` for extracting whitespace-delimited portions
  with the more general `Simple`, which allows for other delimiters.
- Added `mock_instant` as a direct development dependency for use in tests.
- Made internal data sources composable, avoiding code duplication.
- Created io-uring with flags that may reduce overall resource usage.
- Reordered specifiers in [README](./README.md) to reflect the order they appear
  in the code parsing them.


## 0.1.0 - 2024-06-01

### Added

- Initial (Rust) version of the `status` utility.
- README, including building and preliminary usage documentation.
