Feature: CLI Contact Management
  As a Signal TUI user
  I want to manage contacts via CLI
  So that I can view and sync my contact list

  Background:
    Given two registered accounts "alice" and "bob" exist
    And account "alice" is active

  Scenario: List contacts
    When I run "list-contacts"
    Then I should see contact "bob" in the output
    And the contact should have a UUID

  Scenario: Sync contacts
    When I run "sync-contacts"
    Then contacts should be synchronized successfully