Feature: CLI Messaging
  As a Signal TUI user
  I want to send and receive messages via CLI
  So that I can communicate without the TUI interface

  Background:
    Given two registered accounts "alice" and "bob" exist
    And account "alice" is active

  Scenario: Send a text message
    When I run "send-message --recipient bob --text-message 'Hello from Alice'"
    Then the message should be sent successfully
    And account "bob" is active
    And account "bob" should receive "Hello from Alice" from "alice"

  Scenario: Send a message with attachment
    Given a test file "test.txt" exists
    When I run "send-attachment --recipient bob --attachment-path test.txt --text-message 'Check this'"
    Then the attachment should be sent successfully
    And account "bob" should receive an attachment "test.txt" from "alice"

  Scenario: List messages from contact
    Given account "bob" sent "Hi Alice" to "alice"
    When I run "list-messages --contact bob"
    Then I should see "Hi Alice" in the output
    And the sender should be "bob"

  Scenario: List messages with timestamp filter
    Given account "bob" is active
    And I sent "Old message" to "alice" at timestamp "1000000"
    And I sent "New message" to "alice" at timestamp "2000000"
    When account "alice" is active
    And I run "list-messages --contact bob 1500000"
    Then I should see "New message" in the output
    And I should not see "Old message" in the output

  Scenario: Receive messages
    Given account "bob" sent "New message" to "alice"
    When I run "receive"
    Then I should see "New message" in the output
    And the sender should be "bob"