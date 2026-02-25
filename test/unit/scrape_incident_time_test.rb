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

end
