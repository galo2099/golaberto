require "test_helper"
require "scrape"

class ScrapeIncidentTimeTest < ActiveSupport::TestCase
  test "incident_minute ignores added time" do
    incident = { "time" => 90, "addedTime" => 3 }

    assert_equal 90, incident_minute(incident)
  end

  test "incident_extra_time does not treat second half stoppage as aet" do
    incident = { "time" => 90, "addedTime" => 4 }

    assert_not incident_extra_time?(incident)
  end

  test "incident_extra_time does not treat first half stoppage as aet" do
    incident = { "time" => 45, "addedTime" => 5 }

    assert_equal 45, incident_minute(incident)
    assert_not incident_extra_time?(incident)
  end

  test "incident_extra_time marks extra time periods" do
    incident = { "time" => 105, "addedTime" => 1 }

    assert incident_extra_time?(incident)
    assert_equal 105, incident_minute(incident)
  end

  test "incident_minute uses timeSeconds fallback" do
    incident = { "timeSeconds" => 59 * 60 }

    assert_equal 60, incident_minute(incident)
  end

  test "incident_extra_time uses timeSeconds fallback for extra time detection" do
    incident = { "timeSeconds" => 91 * 60 }

    assert_equal 92, incident_minute(incident)
    assert incident_extra_time?(incident)
  end

  test "incident_player_id returns nil when nested player hash is missing" do
    incident = { "incidentType" => "substitution", "playerIn" => { "id" => 10 } }

    assert_nil incident_player_id(incident, "playerOut")
  end

  test "incident_player_id returns id when nested player hash exists" do
    incident = { "playerOut" => { "id" => 42 } }

    assert_equal 42, incident_player_id(incident, "playerOut")
  end

  test "incident_player_out_name supports legacy pl_name_o" do
    incident = { "pl_name_o" => "Old Name" }

    assert_equal "Old Name", incident_player_out_name(incident)
  end

  test "incident_player_out_name supports playerNameOut" do
    incident = { "playerNameOut" => "New Name" }

    assert_equal "New Name", incident_player_out_name(incident)
  end
end
