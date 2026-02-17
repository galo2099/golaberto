class OddsHistoryBackfillService
  LOCK_PATH = Rails.root.join("tmp", "odds_history_backfill.lock")

  def self.start_async(championship_id:, phase_id: nil, group_id: nil, from_date: nil, to_date: nil, reset: false)
    lock_file = File.open(LOCK_PATH, "w")
    unless lock_file.flock(File::LOCK_EX | File::LOCK_NB)
      lock_file.close
      return :busy
    end

    Thread.new do
      ActiveRecord::Base.connection_pool.with_connection do
        begin
          run_unlocked(
            championship_id: championship_id,
            phase_id: phase_id,
            group_id: group_id,
            from_date: from_date,
            to_date: to_date,
            reset: reset,
          )
        ensure
          lock_file.flock(File::LOCK_UN) rescue nil
          lock_file.close rescue nil
        end
      end
    end

    :started
  end

  def self.run(championship_id:, phase_id: nil, group_id: nil, from_date: nil, to_date: nil, reset: false)
    lock_file = File.open(LOCK_PATH, "w")
    unless lock_file.flock(File::LOCK_EX | File::LOCK_NB)
      lock_file.close
      raise "Odds history backfill is already running"
    end

    begin
      run_unlocked(
        championship_id: championship_id,
        phase_id: phase_id,
        group_id: group_id,
        from_date: from_date,
        to_date: to_date,
        reset: reset,
      )
    ensure
      lock_file.flock(File::LOCK_UN) rescue nil
      lock_file.close rescue nil
    end
  end

  def self.run_unlocked(championship_id:, phase_id: nil, group_id: nil, from_date: nil, to_date: nil, reset: false)
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

      played_game_days = base_games_json
        .select { |game| game["played"] && game["date"].present? }
        .map { |game| Date.parse(game["date"].to_s) }
        .uniq
        .sort

      game_days = played_game_days.dup
      if played_game_days.any?
        first_snapshot_day = played_game_days.first - 1.day
        game_days.unshift(first_snapshot_day)
      end

      game_days = game_days.select { |day| day >= from_date } if from_date
      game_days = game_days.select { |day| day <= to_date } if to_date

      if game_days.empty?
        puts "   no played game dates found, skipping"
        next
      end

      game_days.each_with_index do |day, idx|
        games_with_state = base_games_json.map do |game|
          game_date = game["date"].present? ? Date.parse(game["date"].to_s) : nil
          should_be_played = game["played"] && game_date.present? && game_date <= day

          game.merge(
            "_snapshot_date" => game_date,
            "_snapshot_played" => should_be_played,
          )
        end

        first_unplayed_game = games_with_state
          .reject { |game| game["_snapshot_played"] }
          .min_by { |game| [ game["_snapshot_date"] || Date.new(9999, 12, 31), game["id"] ] }

        games_json_for_day = games_with_state.map do |game|
          home_power = if !game["_snapshot_played"] && first_unplayed_game.present?
            first_unplayed_game["home_power"]
          else
            game["home_power"]
          end

          away_power = if !game["_snapshot_played"] && first_unplayed_game.present?
            first_unplayed_game["away_power"]
          else
            game["away_power"]
          end

          {
            "id" => game["id"],
            "home_id" => game["home_id"],
            "away_id" => game["away_id"],
            "home_score" => game["home_score"],
            "away_score" => game["away_score"],
            "played" => game["_snapshot_played"],
            "home_power" => home_power,
            "away_power" => away_power,
          }
        end

        group.odds(
          games_json: games_json_for_day,
          snapshot_time: day.end_of_day,
          persist_game_importance: false,
          persist_team_odds: false,
          persist_group_progress: false
        )

        puts "   [#{idx + 1}/#{game_days.size}] #{day}"
      end

      puts "   finished backfill up to #{game_days.last}"
    end
  end
end
