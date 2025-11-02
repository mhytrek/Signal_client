Feature: CLI Profile Management
  As a Signal TUI user
  I want to view my profile via CLI
  So that I can check my Signal profile information

  Background:
    Given two registered accounts "alice" and "bob" exist
    And account "alice" is active

  Scenario: Get profile
    When I run "get-profile"
    Then I should see profile information
    And I should see profile name or "N/A"
    And I should see unrestricted access status