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

    # Create 6 teams for num_ranks = 6
    @graph_teams = []
    (1..6).each do |i|
      team = Team.create!(name: "GraphTeam #{i}", country: "GT#{i}")
      @graph_teams << team
      TeamGroup.create!(group: graph_group, team: team)
    end
    team_g_a = @graph_teams.first # Team to visit its page

    # Define group.zones (ensure group is reloaded if teams were just added)
    graph_group.reload
    # Order of zones in this array is important for the greedy lane allocation.
    graph_group.zones = [
      { 'name' => 'ZoneA Green', 'color' => 'rgb(0,255,0)', 'position' => [1,2] },
      { 'name' => 'ZoneB Blue',  'color' => 'rgb(0,0,255)', 'position' => [3,4] },
      { 'name' => 'ZoneC Red',   'color' => 'rgb(255,0,0)', 'position' => [1,3] },
      { 'name' => 'ZoneD Yellow','color' => 'rgb(255,255,0)', 'position' => [5,6] }
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
    # 4. Assertions for 'Connected Components' Logic
    assert_equal 8, markings_data.count, "Unexpected total number of markings"
    graph_max_y = 101.0
    
    # Expected: C1 = [ZoneA, ZoneB, ZoneC], C2 = [ZoneD]
    # num_overall_groups = 2
    height_per_group = graph_max_y / 2.0 # 50.5

    # C1: 3 zones (A, B, C based on original_idx)
    sub_lane_height_C1 = height_per_group / 3.0 # approx 16.83
    # C2: 1 zone (D)
    sub_lane_height_C2 = height_per_group / 1.0 # 50.5

    # Helper to find a specific marking more precisely
    find_marking_detail = ->(rank_val, y_from_expected, y_to_expected, color_expected) {
      markings_data.find { |m|
        (m["xaxis"]["from"] - (rank_val - 0.5)).abs < 0.001 &&
        (m["xaxis"]["to"] - (rank_val + 0.5)).abs < 0.001 &&
        (m["yaxis"]["from"] - y_from_expected).abs < 0.001 &&
        (m["yaxis"]["to"] - y_to_expected).abs < 0.001 &&
        m["color"] == color_expected
      }
    }

    # --- Rank 1 (xaxis: from 0.5, to 1.5) ---
    # C1, ZoneA (Green, original_idx 0 -> sub-lane 0 of C1)
    # C1, ZoneC (Red,   original_idx 2 -> sub-lane 2 of C1)
    r1_markings_count = markings_data.count { |m| (m["xaxis"]["from"] - 0.5).abs < 0.001 }
    assert_equal 2, r1_markings_count, "Expected 2 markings for Rank 1"

    m_r1_c1_za = find_marking_detail.call(1, 0 * sub_lane_height_C1, 1 * sub_lane_height_C1, 'rgb(0,255,0)')
    assert_not_nil m_r1_c1_za, "Rank 1, C1-ZoneA (Green) not found"
    
    m_r1_c1_zc = find_marking_detail.call(1, 2 * sub_lane_height_C1, 3 * sub_lane_height_C1, 'rgb(255,0,0)')
    assert_not_nil m_r1_c1_zc, "Rank 1, C1-ZoneC (Red) not found"

    # --- Rank 2 (xaxis: from 1.5, to 2.5) ---
    # C1, ZoneA (Green, original_idx 0 -> sub-lane 0 of C1)
    r2_markings_count = markings_data.count { |m| (m["xaxis"]["from"] - 1.5).abs < 0.001 }
    assert_equal 1, r2_markings_count, "Expected 1 marking for Rank 2"
    m_r2_c1_za = find_marking_detail.call(2, 0 * sub_lane_height_C1, 1 * sub_lane_height_C1, 'rgb(0,255,0)')
    assert_not_nil m_r2_c1_za, "Rank 2, C1-ZoneA (Green) not found"

    # --- Rank 3 (xaxis: from 2.5, to 3.5) ---
    # C1, ZoneB (Blue, original_idx 1 -> sub-lane 1 of C1)
    # C1, ZoneC (Red,  original_idx 2 -> sub-lane 2 of C1)
    r3_markings_count = markings_data.count { |m| (m["xaxis"]["from"] - 2.5).abs < 0.001 }
    assert_equal 2, r3_markings_count, "Expected 2 markings for Rank 3"
    m_r3_c1_zb = find_marking_detail.call(3, 1 * sub_lane_height_C1, 2 * sub_lane_height_C1, 'rgb(0,0,255)')
    assert_not_nil m_r3_c1_zb, "Rank 3, C1-ZoneB (Blue) not found"
    m_r3_c1_zc = find_marking_detail.call(3, 2 * sub_lane_height_C1, 3 * sub_lane_height_C1, 'rgb(255,0,0)')
    assert_not_nil m_r3_c1_zc, "Rank 3, C1-ZoneC (Red) not found"

    # --- Rank 4 (xaxis: from 3.5, to 4.5) ---
    # C1, ZoneB (Blue, original_idx 1 -> sub-lane 1 of C1)
    r4_markings_count = markings_data.count { |m| (m["xaxis"]["from"] - 3.5).abs < 0.001 }
    assert_equal 1, r4_markings_count, "Expected 1 marking for Rank 4"
    m_r4_c1_zb = find_marking_detail.call(4, 1 * sub_lane_height_C1, 2 * sub_lane_height_C1, 'rgb(0,0,255)')
    assert_not_nil m_r4_c1_zb, "Rank 4, C1-ZoneB (Blue) not found"

    # --- Rank 5 (xaxis: from 4.5, to 5.5) ---
    # C2, ZoneD (Yellow, original_idx 3 -> sub-lane 0 of C2)
    r5_markings_count = markings_data.count { |m| (m["xaxis"]["from"] - 4.5).abs < 0.001 }
    assert_equal 1, r5_markings_count, "Expected 1 marking for Rank 5"
    y_c2_start = height_per_group # Start of Component 2's y-space
    m_r5_c2_zd = find_marking_detail.call(5, y_c2_start + 0 * sub_lane_height_C2, y_c2_start + 1 * sub_lane_height_C2, 'rgb(255,255,0)')
    assert_not_nil m_r5_c2_zd, "Rank 5, C2-ZoneD (Yellow) not found"

    # --- Rank 6 (xaxis: from 5.5, to 6.5) ---
    # C2, ZoneD (Yellow, original_idx 3 -> sub-lane 0 of C2)
    r6_markings_count = markings_data.count { |m| (m["xaxis"]["from"] - 5.5).abs < 0.001 }
    assert_equal 1, r6_markings_count, "Expected 1 marking for Rank 6"
    m_r6_c2_zd = find_marking_detail.call(6, y_c2_start + 0 * sub_lane_height_C2, y_c2_start + 1 * sub_lane_height_C2, 'rgb(255,255,0)')
    assert_not_nil m_r6_c2_zd, "Rank 6, C2-ZoneD (Yellow) not found"
  end
end
