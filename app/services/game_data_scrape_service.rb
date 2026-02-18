require 'scrape'

class GameDataScrapeService
  LOCK_PATH = Rails.root.join("tmp", "game_data_scrape.lock")
  MAX_CONCURRENCY = 3

  def self.start_async(refetch: false)
    lock_file = File.open(LOCK_PATH, "w")
    unless lock_file.flock(File::LOCK_EX | File::LOCK_NB)
      lock_file.close
      return :busy
    end

    Thread.new do
      ActiveRecord::Base.connection_pool.with_connection do
        begin
          run_unlocked(refetch: refetch)
        ensure
          lock_file.flock(File::LOCK_UN) rescue nil
          lock_file.close rescue nil
        end
      end
    end

    :started
  end

  def self.run(refetch: false)
    lock_file = File.open(LOCK_PATH, "w")
    unless lock_file.flock(File::LOCK_EX | File::LOCK_NB)
      lock_file.close
      raise "Game data scrape is already running"
    end

    begin
      run_unlocked(refetch: refetch)
    ensure
      lock_file.flock(File::LOCK_UN) rescue nil
      lock_file.close rescue nil
    end
  end

  def self.phases_to_scrape
    Phase
      .joins(:championship)
      .where.not(scrape_url: [nil, ""])
      .where("championships.end >= ?", Date.today - 30.days)
      .distinct
      .select { |phase| phase.games.where(played: false).where("date < ?", Time.now).exists? }
  end

  def self.scrape_phase(phase, rounds: nil, refetch: false)
    puts "-> Scraping phase ##{phase.id} (#{phase.name}) url=#{phase.scrape_url} rounds=#{rounds.inspect} refetch=#{refetch}"
    options = {}
    options[:rounds] = rounds if rounds
    options[:refetch] = true if refetch
    scrape(phase.id, phase.scrape_url, options)
    puts "   done phase ##{phase.id}"
  rescue => e
    puts "   ERROR phase ##{phase.id}: #{e.class}: #{e.message}"
  end

  def self.scrape_phase_async(phase, rounds: nil, refetch: false)
    Thread.new do
      ActiveRecord::Base.connection_pool.with_connection do
        scrape_phase(phase, rounds: rounds, refetch: refetch)
      end
    end
    :started
  end

  def self.run_unlocked(refetch: false)
    phases = phases_to_scrape
    puts "Found #{phases.size} phase(s) with pending games to scrape (refetch=#{refetch})"

    queue = Queue.new
    phases.each { |phase| queue << phase }

    workers = [MAX_CONCURRENCY, phases.size].min.times.map do
      Thread.new do
        ActiveRecord::Base.connection_pool.with_connection do
          while (phase = queue.pop(true) rescue nil)
            scrape_phase(phase, refetch: refetch)
          end
        end
      end
    end

    workers.each(&:join)
    puts "Game data scrape finished"

    begin
      puts "Updating team ratings..."
      Team.update_ratings
      puts "Team ratings updated"
    rescue => e
      puts "ERROR updating team ratings: #{e.class}: #{e.message}"
    end

    begin
      puts "Updating group odds for recently played games..."
      Game.where(played: true).where("date > ?", DateTime.now - 1.hour)
        .map { |g| g.phase }.sort.uniq
        .each do |phase|
          phase.groups.each do |g|
            g.odds
            g.odds_progress = nil
            g.save!
          end
        end
      puts "Group odds updated"
    rescue => e
      puts "ERROR updating group odds: #{e.class}: #{e.message}"
    end
  end
end
