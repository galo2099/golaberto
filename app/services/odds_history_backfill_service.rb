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

  def self.left_advantage_for(home_field)
    case home_field.to_s
    when "left", "0"
      Game::HOME_ADV
    when "neutral", "1"
      0.0
    when "right", "2"
      -Game::HOME_ADV
    else
      Game::HOME_ADV
    end
  end

  def self.calculate_powers_for_snapshot(home_rating:, away_rating:, home_field:)
    return [ nil, nil ] if home_rating.nil? || away_rating.nil?

    left_advantage = left_advantage_for(home_field)
    avg_base = Game::AVG_BASE

    home_power = [10.0, [0.01, (home_rating.off_rating.to_f - avg_base) / (avg_base * 0.424 + 0.548) * ([0.25, (away_rating.def_rating.to_f + left_advantage) * 0.424 + 0.548].max) + (away_rating.def_rating.to_f + left_advantage)].max].min
    away_power = [10.0, [0.01, (away_rating.off_rating.to_f - avg_base) / (avg_base * 0.424 + 0.548) * ([0.25, (home_rating.def_rating.to_f - left_advantage) * 0.424 + 0.548].max) + (home_rating.def_rating.to_f - left_advantage)].max].min

    [ home_power, away_power ]
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
        only: [ :id, :home_id, :away_id, :home_score, :away_score, :played, :date, :home_field ]
      )

      played_game_days = base_games_json
        .select { |game| game["played"] && game["date"].present? }
        .map { |game| Date.parse(game["date"].to_s) }
        .uniq
        .sort

      team_ids = base_games_json.flat_map { |game| [ game["home_id"], game["away_id"] ] }.compact.uniq
      ratings_by_team = HistoricalRating.where(team_id: team_ids).order(:measure_date).group_by(&:team_id)

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

        last_played_date = games_with_state
          .select { |game| game["_snapshot_played"] }
          .map { |game| game["_snapshot_date"] }
          .compact
          .max
        rating_reference_date = last_played_date.present? ? (last_played_date + 1.day) : (day + 1.day)

        games_json_for_day = games_with_state.map do |game|
          home_power = game["home_power"]
          away_power = game["away_power"]

          unless game["_snapshot_played"]
            home_rating = ratings_by_team.fetch(game["home_id"], []).select { |rating| rating.measure_date < rating_reference_date }.max_by(&:measure_date)
            away_rating = ratings_by_team.fetch(game["away_id"], []).select { |rating| rating.measure_date < rating_reference_date }.max_by(&:measure_date)
            snapshot_home_power, snapshot_away_power = calculate_powers_for_snapshot(
              home_rating: home_rating,
              away_rating: away_rating,
              home_field: game["home_field"],
            )

            home_power = snapshot_home_power unless snapshot_home_power.nil?
            away_power = snapshot_away_power unless snapshot_away_power.nil?
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
