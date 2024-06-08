# Changelog

All notable changes to this project will be documented in this file. Its format
is based on https://keepachangelog.com/en/1.1.0/.

## Unreleased

### Added

- Add tests for internal entry structures.

### Changed

- Make internal data sources composable, avoiding code duplication.
- Create io-uring with flags that may reduce overall resource usage.
- Reorder specifiers in [README](./README.md) to reflect the order they appear in the code
  parsing them.


## 0.1.0 - 2024-06-01

### Added

- Initial (Rust) version of the `status` utility.
- README, including building and preliminary usage documentation.
