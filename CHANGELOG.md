# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.25](https://github.com/edera-dev/krata/compare/v0.0.24...v0.0.25) - 2025-01-07

### Other

- update Cargo.toml dependencies

## [0.0.24](https://github.com/edera-dev/krata/compare/v0.0.23...v0.0.24) - 2024-12-14

### Added

- *(xen)* update xenclient and xenplatform to the latest structure (#433)
- *(xencall)* improve asynchronous support (#430)
- *(evtchn)* harden evtchn handling and improve api (#431)
- *(xenstore)* multi-watch and maybe-commit support (#429)

### Fixed

- *(xenclient)* examples should use supported platform
- *(xenclient)* boot example should use unsupported platform on aarch64
- *(xenplatform)* e820 sanitize should now produce valid mappings
- *(xenplatform)* use cfg attributes for returning supported platforms

### Other

- *(deps)* upgrade dependencies and clean code (#432)
- update Cargo.toml dependencies

## [0.0.23](https://github.com/edera-dev/krata/compare/v0.0.22...v0.0.23) - 2024-09-17

### Other

- update Cargo.toml dependencies

## [0.0.22](https://github.com/edera-dev/krata/compare/v0.0.21...v0.0.22) - 2024-09-16

### Other

- update Cargo.toml dependencies
- preparations for xen control-plane
