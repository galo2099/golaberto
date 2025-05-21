require "application_system_test_case"

class GroupsTest < ApplicationSystemTestCase
  setup do
    @category = categories(:one) # From test/fixtures/categories.yml
    @championship = championships(:championship_with_phases_and_groups) # Assuming this fixture sets up a basic champ
    # If not, create one:
    # @championship = Championship.create!(name: "Test System Championship", category: @category, region: :national, begin: Date.today, end: Date.today + 1.month, point_win: 3, point_draw: 1, point_loss: 0)

    @phase = phases(:phase_with_groups) # Assuming this fixture exists and is linked to @championship
    # If not, create one:
    # @phase = Phase.create!(name: "Test System Phase", championship: @championship, order_by: 1, sort: "pt,gd")

    @group = groups(:group_one) # Assuming this fixture exists and is linked to @phase
    # If not, create one:
    # @group = Group.create!(name: "Test System Group", phase: @phase)

    # Ensure the group is associated with the correct phase and championship for route helpers
    # This might be redundant if fixtures are well-defined, but good for safety.
    @phase.championship = @championship
    @phase.save!
    @group.phase = @phase
    @group.save!


    # Create Teams for the group if not already handled by fixtures
    @teams = []
    if @group.teams.count < 5 # For defining_overlapping_zones test
      (5 - @group.teams.count).times do |i|
        team = Team.create!(name: "Sys Team #{i+1}", country: "SysCountry#{i+1}")
        @teams << team
        TeamGroup.create!(group: @group, team: team)
      end
    else
      @teams = @group.teams.limit(5).to_a
    end
  end

  test "defining overlapping zones in group edit UI" do
    visit edit_group_path(@group)

    # Click 'Create a new zone' twice
    2.times { click_button "Create a new zone" }

    # Zone 1
    within "#zones li:nth-child(1)" do # Assuming new zones are appended
      fill_in "Name", with: "Zone Alpha"
      # For Spectrum color picker, find the input and set its value
      # This assumes the input has an ID like 'group_zones_0_color' or similar
      # We might need to inspect the generated HTML to get the exact ID or a more robust selector.
      # For now, let's assume a common pattern for the input associated with spectrum.
      # If the input is directly visible and takes hex:
      # fill_in "Color", with: "#FF0000"
      # If it's hidden and updated by spectrum, we need to target the correct hidden input.
      # Let's assume the input field for color is identifiable.
      # The actual ID might be like 'group_zones_X_color' where X is an index.
      # Since we are adding new zones, these indices might be dynamic.
      # A more robust way would be to target by name attribute if IDs are too dynamic.
      find("input[name='group[zones][][color]']", match: :first).set("#FF0000") # Red

      check "1"
      check "2"
      check "3"
    end

    # Zone 2
    within "#zones li:nth-child(2)" do
      fill_in "Name", with: "Zone Beta"
      all("input[name='group[zones][][color]']").last.set("#00FF00") # Green

      check "3" # Overlapping position
      check "4"
    end

    click_button "Update"

    @group.reload
    assert_equal 2, @group.zones.count

    # Sort by name to have a predictable order for assertions
    sorted_zones = @group.zones.sort_by { |z| z['name'] }

    zone_alpha = sorted_zones.find { |z| z['name'] == 'Zone Alpha' }
    zone_beta = sorted_zones.find { |z| z['name'] == 'Zone Beta' }

    assert_not_nil zone_alpha, "Zone Alpha not found"
    assert_equal ['1', '2', '3'], zone_alpha['position'].map(&:to_s).sort
    assert_equal "#FF0000", zone_alpha['color']

    assert_not_nil zone_beta, "Zone Beta not found"
    assert_equal ['3', '4'], zone_beta['position'].map(&:to_s).sort
    assert_equal "#00FF00", zone_beta['color']
  end


  test "displaying overlapping zones in championship table" do
    # Fresh group for this test to avoid interference
    @display_phase = Phase.create!(name: "Display Phase", championship: @championship, order_by: 2, sort: "pt,gd")
    @display_group = Group.create!(name: "Display Group", phase: @display_phase)

    # Create 3 teams
    team_a = Team.create!(name: "Team AlphaDisplay", country: "DA")
    team_b = Team.create!(name: "Team BetaDisplay", country: "DB")
    team_c = Team.create!(name: "Team GammaDisplay", country: "DC")

    # Add teams to group
    TeamGroup.create!(group: @display_group, team: team_a, add_sub: 6) # 6 pts
    TeamGroup.create!(group: @display_group, team: team_b, add_sub: 3) # 3 pts
    TeamGroup.create!(group: @display_group, team: team_c, add_sub: 0) # 0 pts
    
    # Manually set up games to enforce order if add_sub is not enough
    # Game 1: A vs B (A wins) -> A=3, B=0
    Game.create!(phase: @display_phase, home: team_a, away: team_b, home_score: 1, away_score: 0, played: true, date: Date.today - 3.days)
    # Game 2: A vs C (A wins) -> A=6, C=0
    Game.create!(phase: @display_phase, home: team_a, away: team_c, home_score: 1, away_score: 0, played: true, date: Date.today - 2.days)
    # Game 3: B vs C (B wins) -> B=3, C=0
    Game.create!(phase: @display_phase, home: team_b, away: team_c, home_score: 1, away_score: 0, played: true, date: Date.today - 1.day)

    @display_group.zones = [
      { 'name' => 'Top Tier', 'color' => 'rgb(0, 255, 0)', 'position' => [1, 2] }, # Green
      { 'name' => 'Mid Tier', 'color' => 'rgb(0, 0, 255)', 'position' => [2, 3] }  # Blue
    ]
    @display_group.save!
    
    # Ensure team_table calculates correctly - this might require some waiting or a refresh
    # For system tests, visiting the page usually triggers all necessary calculations.

    visit championship_path(@championship) # Or phase_path(@display_phase) or group_path(@display_group) if those are where the table is rendered

    # Find the table for @display_group specifically if multiple groups are on the page
    group_table_selector = "div:has(h3 > span > a[href*='#{group_path(@display_group)}']) table.class_table"
    # If the link is not to group_path, adjust: e.g. to find by group name text
    # group_table_selector = "div:has(h3:contains('#{@display_group.name}')) table.class_table"


    within find(group_table_selector) do
      # Team A (Pos 1)
      team_a_row = find("tr:has(td.name:contains('#{team_a.name}'))")
      assert_match /background-color: rgb\(0, 255, 0\);/, team_a_row['style'], "Team A row background color mismatch"
      pos_1_cell = team_a_row.find("td.pos")
      assert_equal "1", pos_1_cell.text.strip # Check position number, ensure no markers
      assert_not pos_1_cell.has_css?("span.additional-zone-marker"), "Team A (Pos 1) should have no additional markers"

      # Team B (Pos 2)
      team_b_row = find("tr:has(td.name:contains('#{team_b.name}'))")
      assert_match /background-color: rgb\(0, 255, 0\);/, team_b_row['style'], "Team B row background color mismatch (should be Top Tier)"
      pos_2_cell = team_b_row.find("td.pos")
      assert_match /^2\s/, pos_2_cell.text.strip, "Team B (Pos 2) cell should start with '2'"
      marker = pos_2_cell.find("span.additional-zone-marker")
      assert_match /background-color: rgb\(0, 0, 255\);/, marker['style'], "Team B marker color mismatch"
      assert_equal "Mid Tier", marker['title'], "Team B marker title mismatch"

      # Team C (Pos 3)
      team_c_row = find("tr:has(td.name:contains('#{team_c.name}'))")
      assert_match /background-color: rgb\(0, 0, 255\);/, team_c_row['style'], "Team C row background color mismatch"
      pos_3_cell = team_c_row.find("td.pos")
      assert_equal "3", pos_3_cell.text.strip # Check position number, ensure no markers
      assert_not pos_3_cell.has_css?("span.additional-zone-marker"), "Team C (Pos 3) should have no additional markers"
    end
  end
end
