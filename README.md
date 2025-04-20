<h1 align="center">Signal Client</h1>

## About the Project

### Description
A desktop client for Signal with a terminal interface, providing basic functionalities. It is designed to run on Linux, Windows, and macOS platforms.

### Built with
![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)

---

## Getting Started

To ensure the best experience, follow the guide below:

### Installation Guide

1. **Install Rust and Cargo**  
   Follow the official guide to install Rust and Cargo:  
   [Rust Installation Guide](https://www.rust-lang.org/tools/install)

2. **Install `protoc`**  
   Follow the official guide to install `protoc`:  
   [Protoc Installation Guide](https://protobuf.dev/installation/)

3. **Download the Latest Release**
  * 3.1. Navigate to the project's releases page.
  * 3.2. Download the latest tagged release suitable for your operating system.
  * 3.3. Extract the contents to your desired directory.

4. **Run the Project**
  * 4.1. Open a terminal or command prompt.
  * 4.2. Navigate to the project's directory:
  * 4.3. Run the project using Cargo:  
    `cargo run`

---

## Usage
```bash
cargo run <command>
```

## Functionalities

### **link-device**
Links this device to your Signal account.
```bash
cargo run link-device --device-name "MyDevice"
```


### **sync-contacts**
Synchronizes contacts with the primary device.
```bash
cargo run sync-contacts
```


### **list-contacts**
Prints the locally stored contacts.
```bash
cargo run list-contacts
```


### **run-app**
Displays a prototype layout with example data.
- Allows registering new device if it's not already registered
- Includes synchronization of the contact list (based on UUID).
- Allows sending messages using the UUID of a contact.
  
```bash
cargo run run-app
```

Functionalities in app:
- linking new device 
- sync contacts (in the background)
- sending message (using UUID) ~ UUID can be checked using `cargo run list-contacts`


### **send-message**
Sends a text message.

```bash
cargo run send-message --recipient "recipient_uuid" "Hello, this is a test message!"
```


### **help**
Prints this help message or details for specific subcommands.
```bash
cargo run --help
```

---

## Authors
- Ciepiela Ida
- Hytrek Michalina
- Rękas Jakub

## License
?

## Acknowledgments
Project made as a matter of Bachelor Thesis @ AGH UST under the guidance of @kpietak
