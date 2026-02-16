namespace :odds_history do
  desc "Backfill daily odds history for championship groups. Usage: rake odds_history:backfill CHAMPIONSHIP_ID=1 [PHASE_ID=2] [GROUP_ID=3] [FROM=2024-01-01] [TO=2024-12-31] [RESET=true]"
  task backfill: :environment do
    championship_id = ENV["CHAMPIONSHIP_ID"]
    phase_id = ENV["PHASE_ID"]
    group_id = ENV["GROUP_ID"]
    from_date = ENV["FROM"].present? ? Date.parse(ENV["FROM"]) : nil
    to_date = ENV["TO"].present? ? Date.parse(ENV["TO"]) : nil
    reset = ENV["RESET"].to_s == "true"

    if championship_id.blank?
      raise "CHAMPIONSHIP_ID is required"
    end

    groups = Group.joins(:phase).
      where(phases: { championship_id: championship_id }).
      includes(:team_groups, :games)
    groups = groups.where(phase_id: phase_id) if phase_id.present?
    groups = groups.where(id: group_id) if group_id.present?

    puts "Backfilling odds history for #{groups.size} group(s)"

    groups.find_each do |group|
      puts "-> Group ##{group.id} (#{group.name})"

      group.team_groups.each do |team_group|
        team_group.odds_histories.delete_all if reset
      end

      games = group.games.select(:id, :date, :played).to_a
      original_played_by_id = games.each_with_object({}) { |game, hash| hash[game.id] = game.played }
      played_game_ids = original_played_by_id.select { |_, played| played }.keys

      game_days = games.
        select { |game| game.date.present? && original_played_by_id[game.id] }.
        map(&:date).
        uniq.
        sort

      if from_date
        game_days = game_days.select { |day| day >= from_date }
      end
      if to_date
        game_days = game_days.select { |day| day <= to_date }
      end

      if game_days.empty?
        puts "   no played game dates found, skipping"
        next
      end

      begin
        game_days.each_with_index do |day, idx|
          past_ids = games.select { |game| game.date.present? && game.date <= day && original_played_by_id[game.id] }.map(&:id)
          future_ids = played_game_ids - past_ids

          unless past_ids.empty?
            Game.where(id: past_ids).update_all(played: true)
          end
          unless future_ids.empty?
            Game.where(id: future_ids).update_all(played: false)
          end

          group.reload
          group.odds(snapshot_time: day.end_of_day)

          puts "   [#{idx + 1}/#{game_days.size}] #{day}"
        end
      ensure
        true_ids = original_played_by_id.select { |_, played| played }.keys
        false_ids = original_played_by_id.select { |_, played| !played }.keys
        Game.where(id: true_ids).update_all(played: true) unless true_ids.empty?
        Game.where(id: false_ids).update_all(played: false) unless false_ids.empty?
      end

      group.reload
      group.odds
      puts "   finished and restored current odds"
    end
  end
end
