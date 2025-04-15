<h1 align="center">Signal Client</h1>

## About the project

### Description
A desktop client for Signal with a terminal interface, provides basic functionalities and is designed to run on Linux, Windows, and macOS platforms.


### Built with
![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)

## Getting Started
To have the best experience follow the guide bellow :)

### Prerequisites
To use the application, you need to install:
- [Rust]()
- [Protoc]()
- [Cargo]()

### Installation

To install, download the latest release tag from the project's repository and follow the provided instructions.

## Usage
```bash
cargo run <command>
```

## Functionalities
### Commands

#### link-device
Links this device to your Signal account.
`cargo run link-device --device-name "MyDevice"`

#### sync-contacts
Synchronizes contacts with the primary device.
`cargo run sync-contacts`

#### list-contacts
Prints the locally stored contacts.
`cargo run list-contacts`

#### run-app
Displays a prototype layout with example data.
- Includes synchronization of the contact list (based on UUID).
- Allows sending messages using the UUID of a contact (requires fetching the UUID in a separate process using `cargo run list-contacts`).
  `cargo run run-app`

Functionalities in app:
- sync contacts (in the background)
- sending message (using UUID) ~ UUID can be checked using `cargo run list-contacts`

#### send-message
Sends a text message.
(not working! Bug!)
`cargo run send-message --recipient "phone_number/name" "Hello, this is a test message!"`

#### help
Prints this help message or details for specific subcommands.
`cargo run --help`



## Authors
- Ciepiela Ida
- Hytrek Michalina
- Rękas Jakub

## License
?

## Acknowledgments
Project made as a matter of Bachelor Thesis @ AGH UST under the guidance of @kpietak
