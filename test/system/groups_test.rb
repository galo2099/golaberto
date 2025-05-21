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

  test 'odds_graph_markings_correctly_represent_overlapping_zones' do
    # 1. Setup Data
    # Create a new, isolated group and phase for this test to ensure correct team count (3)
    # and specific zone setup.
    graph_phase = Phase.create!(
      name: "Graph Test Phase",
      championship: @championship, # Use existing championship from setup
      order_by: @phase.order_by + 1, # Ensure unique order_by
      sort: "pt,gd"
    )
    graph_group = Group.create!(
      name: "Graph Test Group",
      phase: graph_phase
    )

    # Create 3 teams
    team_g_a = Team.create!(name: "GraphTeam A", country: "GTA")
    team_g_b = Team.create!(name: "GraphTeam B", country: "GTB")
    team_g_c = Team.create!(name: "GraphTeam C", country: "GTC")

    # Add teams to the group
    TeamGroup.create!(group: graph_group, team: team_g_a)
    TeamGroup.create!(group: graph_group, team: team_g_b)
    TeamGroup.create!(group: graph_group, team: team_g_c)

    # Define group.zones (ensure group is reloaded if teams were just added)
    graph_group.reload
    graph_group.zones = [
      { 'name' => 'Zone1 Green', 'color' => 'rgb(0,255,0)', 'position' => [1] },
      { 'name' => 'Zone2 Blue',  'color' => 'rgb(0,0,255)', 'position' => [1, 2] },
      { 'name' => 'Zone3 Red',   'color' => 'rgb(255,0,0)', 'position' => [1, 3] }
    ]
    graph_group.save!

    # 2. Navigate to the page
    # We need to visit the team page of one of the teams in this group.
    # The markings are generated based on @groups[0], which in the context of
    # championship/team.html.erb, is the group the @team belongs to.
    # So, we need to make sure the controller loads `graph_group` as `@groups[0]`.
    # This typically happens if @team is part of that group and that group is the primary one for the view.
    # For simplicity, we assume team_g_a is the @team for the page view.
    # The controller logic usually sets @groups = @team.groups.includes(:phase).order('phases.order_by')
    # So, if team_g_a is only in graph_group, graph_group should be @groups[0] if its phase is ordered first
    # or if it's the only group.
    # We'll assign team_g_a's odds for the graph.
    team_group_a = team_g_a.team_groups.find_by(group: graph_group)
    team_group_a.update!(odds: [0.1, 0.2, 0.7]) # Example odds, sum to 1.0 or 100% if needed by graph logic

    visit championship_team_path(@championship, team: team_g_a)

    # 3. Extract Markings Data
    script_content = page.find('script', text: /markings:/, visible: false).text(:all)
    
    # Regex to capture the array part of "markings: [...]"
    # It looks for "markings:", optional whitespace, then captures the content of the array "[...]"
    # It needs to be careful about nested arrays or objects within the markings if any.
    # Assuming markings is a simple array of objects like { xaxis: { from: ..., to: ... }, color: "..." }
    match = /markings:\s*(\[.*?\])/m.match(script_content)
    assert_not_nil match, "Could not find markings data in script tag"
    
    markings_json_string = match[1]
    markings_data = JSON.parse(markings_json_string)

    # 4. Assertions
    # Total expected markings: 3 for Rank 1 (3 segments) + 1 for Rank 2 + 1 for Rank 3 = 5
    assert_equal 5, markings_data.count, "Unexpected total number of markings"

    # Expected order for Rank 1 segments (Blue, Green, Red) due to sort_by { |z| [z["color"], z["name"]] }
    # 'rgb(0,0,255)' (Blue) < 'rgb(0,255,0)' (Green) < 'rgb(255,0,0)' (Red)

    # Rank 1 - Segment 1 (Blue)
    marking_r1_s1 = markings_data.find { |m| m["xaxis"]["from"] == 0.5 && m["color"] == 'rgb(0,0,255)' }
    assert_not_nil marking_r1_s1, "Marking for Rank 1, Segment 1 (Blue) not found"
    assert_in_delta 0.5, marking_r1_s1["xaxis"]["from"]
    assert_in_delta 0.5 + (1.0/3.0), marking_r1_s1["xaxis"]["to"]
    assert_equal 'rgb(0,0,255)', marking_r1_s1["color"]

    # Rank 1 - Segment 2 (Green)
    # Need to find based on 'from' as color alone isn't unique for the whole set
    marking_r1_s2 = markings_data.find { |m| m["color"] == 'rgb(0,255,0)' && m["xaxis"]["from"] > 0.5 && m["xaxis"]["from"] < 1.0 }
    assert_not_nil marking_r1_s2, "Marking for Rank 1, Segment 2 (Green) not found"
    assert_in_delta 0.5 + (1.0/3.0), marking_r1_s2["xaxis"]["from"]
    assert_in_delta 0.5 + 2.0*(1.0/3.0), marking_r1_s2["xaxis"]["to"]
    assert_equal 'rgb(0,255,0)', marking_r1_s2["color"]
    
    # Rank 1 - Segment 3 (Red)
    marking_r1_s3 = markings_data.find { |m| m["color"] == 'rgb(255,0,0)' && m["xaxis"]["from"] > (0.5 + (1.0/3.0)) && m["xaxis"]["from"] < 1.5 }
    assert_not_nil marking_r1_s3, "Marking for Rank 1, Segment 3 (Red) not found"
    assert_in_delta 0.5 + 2.0*(1.0/3.0), marking_r1_s3["xaxis"]["from"]
    assert_in_delta 1.5, marking_r1_s3["xaxis"]["to"]
    assert_equal 'rgb(255,0,0)', marking_r1_s3["color"]

    # Rank 2 (Blue)
    marking_r2 = markings_data.find { |m| m["xaxis"]["from"] == 1.5 }
    assert_not_nil marking_r2, "Marking for Rank 2 (Blue) not found"
    assert_in_delta 1.5, marking_r2["xaxis"]["from"]
    assert_in_delta 2.5, marking_r2["xaxis"]["to"]
    assert_equal 'rgb(0,0,255)', marking_r2["color"]
    
    # Rank 3 (Red)
    marking_r3 = markings_data.find { |m| m["xaxis"]["from"] == 2.5 }
    assert_not_nil marking_r3, "Marking for Rank 3 (Red) not found"
    assert_in_delta 2.5, marking_r3["xaxis"]["from"]
    assert_in_delta 3.5, marking_r3["xaxis"]["to"]
    assert_equal 'rgb(255,0,0)', marking_r3["color"]
  end
end
