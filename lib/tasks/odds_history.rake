namespace :odds_history do
  desc "Backfill daily odds history for championship groups. Usage: rake odds_history:backfill CHAMPIONSHIP_ID=1 [PHASE_ID=2] [GROUP_ID=3] [FROM=2024-01-01] [TO=2024-12-31] [RESET=true]"
  task backfill: :environment do
    championship_id = ENV["CHAMPIONSHIP_ID"]
    phase_id = ENV["PHASE_ID"]
    group_id = ENV["GROUP_ID"]
    from_date = ENV["FROM"].present? ? Date.parse(ENV["FROM"]) : nil
    to_date = ENV["TO"].present? ? Date.parse(ENV["TO"]) : nil
    reset = ENV["RESET"].to_s == "true"

    raise "CHAMPIONSHIP_ID is required" if championship_id.blank?

    groups = Group.joins(:phase)
      .where(phases: { championship_id: championship_id })
      .includes(:team_groups)
    groups = groups.where(phase_id: phase_id) if phase_id.present?
    groups = groups.where(id: group_id) if group_id.present?

    puts "Backfilling odds history for #{groups.size} group(s)"

    groups.find_each do |group|
      puts "-> Group ##{group.id} (#{group.name})"

      if reset
        group.team_groups.find_each { |team_group| team_group.odds_histories.delete_all }
      end

      base_games_json = group.games.includes(:home, :away).as_json(
        methods: [:home_power, :away_power],
        only: [ :id, :home_id, :away_id, :home_score, :away_score, :played, :date ]
      )

      game_days = base_games_json
        .select { |game| game["played"] && game["date"].present? }
        .map { |game| Date.parse(game["date"].to_s) }
        .uniq
        .sort

      game_days = game_days.select { |day| day >= from_date } if from_date
      game_days = game_days.select { |day| day <= to_date } if to_date

      if game_days.empty?
        puts "   no played game dates found, skipping"
        next
      end

      game_days.each_with_index do |day, idx|
        games_json_for_day = base_games_json.map do |game|
          game_date = game["date"].present? ? Date.parse(game["date"].to_s) : nil
          should_be_played = game["played"] && game_date.present? && game_date <= day

          {
            "id" => game["id"],
            "home_id" => game["home_id"],
            "away_id" => game["away_id"],
            "home_score" => game["home_score"],
            "away_score" => game["away_score"],
            "played" => should_be_played,
            "home_power" => game["home_power"],
            "away_power" => game["away_power"],
          }
        end

        group.odds(
          games_json: games_json_for_day,
          snapshot_time: day.end_of_day,
          persist_game_importance: false
        )

        puts "   [#{idx + 1}/#{game_days.size}] #{day}"
      end

      group.odds
      puts "   finished and restored current odds"
    end
  end
end
