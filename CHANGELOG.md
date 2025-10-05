# Signal Client
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)

## [Unreleased] - yyyy-mm-dd

### Breaking
- Migrated from `SledStore` to `SqliteStore`.

### Added
- Retry strategy for sending messages

### Changed

### Fixed
- Timezone in messages timestamps.

## [0.2.0] - 2025-07-08

### Added
- Sending attachments (in cli and ui).
- Receiving messages (in cli and ui).
- Getting profile data (in cli and tui).
- displaying attachments in tui
- saving attachments via tui

### Fixed
- Handling connection issues.
- Refreshing contacts list in ui.

## [0.1.0] - 2025-04-15

First version
