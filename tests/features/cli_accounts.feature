Feature: CLI Account Management
  As a Signal TUI user
  I want to manage multiple accounts via CLI
  So that I can switch between different Signal identities

  Background:
    Given two registered accounts "alice" and "bob" exist

  Scenario: List accounts
    When I run "list-accounts"
    Then I should see account "alice" in the output
    And I should see account "bob" in the output

  Scenario: Get current account
    Given account "alice" is active
    When I run "get-current-account"
    Then I should see "alice" in the output

  Scenario: Switch account
    Given account "alice" is active
    When I run "switch-account --account-name bob"
    Then the current account should be "bob"
    When I run "get-current-account"
    Then I should see "bob" in the output

  Scenario: Create new account
    Given account 'charlie' doesn't exist
    When I link-account --account-name 'charlie' --device-name 'Charlie Device'
    Then I should see QR code linking prompt
    And account "charlie" should be created after linking

  Scenario: Delete account
    When I run "unlink-account --account-name bob"
    And I confirm deletion with "y"
    Then account "bob" should be deleted
    And I should not see "bob" when listing accounts