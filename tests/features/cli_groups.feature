Feature: CLI Group Management
  As a Signal TUI user
  I want to manage groups via CLI
  So that I can view and message groups

  Background:
    Given two registered accounts "alice" and "bob" exist
    And account "alice" is active
    And a group "Test" exists with members "alice" and "bob"

  Scenario: List groups
    When I run "list-groups"
    Then I should see group "Test" in the output

  Scenario: Send message to group
    When I run "send-message-to-group --recipient 'Test' --text-message 'Hello group'"
    Then the message should be sent successfully
    And group "Test" should receive "Hello group"

  Scenario: List group messages
    Given group "Test" has message "Group chat" from "bob"
    When I run "list-messages --group 'Test'"
    Then I should see "Group chat" in the output