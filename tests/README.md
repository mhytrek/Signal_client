# Setting up Cucumber BDD Tests

## Overview

This project uses Cucumber for Behavior-Driven Development (BDD) testing. BDD tests are written in plain language (Gherkin syntax) that describes the behavior of the application from a user's perspective. This makes tests readable by non-technical stakeholders and serves as living documentation.

The test suite verifies end-to-end functionality of the Signal client, including:
- Account management (creating, switching, deleting accounts)
- Sending and receiving messages
- Contact synchronization
- Group messaging
- Attachment handling

## Prerequisites

Before running the tests, you need to set up two Signal accounts with test data.

### Required Setup

1. **Two Signal Accounts**: You'll need access to two phone numbers to register Signal accounts (Bob and Alice)
2. **Signal Official Apps**: Install Signal on two devices (or use Signal Desktop)
3. **Accounts in each other's contacts**: The accounts must have each other added as contacts
4. **A test group**: Create a group named "Test" with both accounts as members

## Step-by-Step Setup

### 1. Build the Project

```bash
cargo build --release
```

### 2. Link Your First Account (Alice)

```bash
cargo run -- link-account --account-name "Alice" --device-name "TestDevice1"
```

Scan the QR code with your first Signal account, then sync contacts:

```bash
cargo run -- sync-contacts
cargo run -- receive
```

### 3. Send Test Messages from Alice

From the official Signal app on the first account:
1. Send at least one message to the second account
2. Send at least one message to the "Test" group

### 4. Link Your Second Account (Bob)

```bash
cargo run -- link-account --account-name "Bob" --device-name "TestDevice2"
```

Scan the QR code with your second Signal account, then sync contacts:

```bash
cargo run -- sync-contacts
cargo run -- receive
```

### 5. Send Test Messages from Bob

From the official Signal app on the second account:
1. Send at least one message to the first account
2. Send at least one message to the "Test" group

### 6. Create Test Fixtures Directory Structure

```bash
mkdir -p tests/fixtures/accounts
```

### 7. Move Account Databases to Fixtures

The accounts are stored in different locations depending on your build mode:

**For development builds:**
```bash
# Find your accounts directory (usually in project root)
mv signal_client/accounts/Alice tests/fixtures/accounts/
mv signal_client/accounts/Bob tests/fixtures/accounts/
```

**For release builds:**
```bash
# Accounts are stored in ~/.local/share/signal_client/accounts
mv ~/.local/share/signal_client/accounts/Alice tests/fixtures/accounts/
mv ~/.local/share/signal_client/accounts/Bob tests/fixtures/accounts/
```

### 8. Get Account UUIDs

You need to retrieve the UUIDs for both accounts. Run these commands:

```bash
# Switch to first account and list contacts
cargo run -- switch-account --account-name "Michalina1"
cargo run -- list-contacts

# Switch to second account and list contacts
cargo run -- switch-account --account-name "Michal"
cargo run -- list-contacts
```

From the output, note:
- The UUID for "Alice" account
- The UUID for "Bob" account
- The contact names each account uses for the other

### 9. Create Test Configuration File

Create `tests/fixtures/test_config.json` with the following structure:

```json
{
  "accounts": {
    "alice": {
      "account_name": "Alice",
      "uuid": "YOUR-FIRST-ACCOUNT-UUID-HERE",
      "contact_name_in_bob": "contact_name_in_bob"
    },
    "bob": {
      "account_name": "Bob",
      "uuid": "YOUR-SECOND-ACCOUNT-UUID-HERE",
      "contact_name_in_alice": "contact_name_in_alice"
    }
  }
}
```

Replace the placeholder values:
- `YOUR-FIRST-ACCOUNT-UUID-HERE` with the actual UUID of the Alice account
- `YOUR-SECOND-ACCOUNT-UUID-HERE` with the actual UUID of the Bob account
- `contact_name_in_bob` with how Alice appears in Bob's contacts
- `contact_name_in_alice` with how Bob appears in Alice's contacts

### 10. Verify Directory Structure

Your test fixtures should look like this:

```
tests/
├── fixtures/
│   ├── accounts/
│   │   ├── Alice/
│   │   │   └── store.db
│   │   └── Bob/
│   │       └── store.db
│   └── test_config.json
└── features/
    └── *.feature files
```

## Running the Tests

### Run All Tests

```bash
cargo test --test cucumber
```

### Run Tests with Output to File

```bash
cargo test --test cucumber 2>&1 | tee test_results.txt
```

## Understanding Test Results

- **Passed tests**: Tests that completed successfully
- **Failed tests**: Tests where assertions failed or errors occurred
- **Skipped tests**: Tests marked to be skipped

The test output will show which steps passed/failed for each scenario, making it easy to identify issues.

## Test Framework Architecture

The test suite uses:
- **Cucumber-rs**: The Rust implementation of Cucumber BDD framework
- **Gherkin syntax**: Human-readable test scenarios in `.feature` files
- **Step definitions**: Rust functions that implement each test step
- **Test fixtures**: Real Signal account databases for integration testing

Each test scenario:
1. Executes CLI commands against the temporary accounts
2. Verifies the output matches expected behavior

## Troubleshooting

### Tests Fail with "Account not found"
- Verify your `test_config.json` has correct account names and UUIDs
- Check that account databases exist in `tests/fixtures/accounts/`

### Database Locking Issues
- Ensure no other Signal client instances are accessing the test databases

### Message Delivery Failures
- Ensure both test accounts are properly linked and have network connectivity
- Verify the "Test" group exists and both accounts are members

## Notes
- Tests use real Signal accounts and make actual API calls
- Rate limiting applies - don't run tests too frequently
- Keep your test account credentials secure
- The fixture databases contain real message data - handle appropriately