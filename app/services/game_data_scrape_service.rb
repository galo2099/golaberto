require 'scrape'

class GameDataScrapeService
  LOCK_PATH = Rails.root.join("tmp", "game_data_scrape.lock")
  MAX_CONCURRENCY = 3

  def self.start_async
    lock_file = File.open(LOCK_PATH, "w")
    unless lock_file.flock(File::LOCK_EX | File::LOCK_NB)
      lock_file.close
      return :busy
    end

    Thread.new do
      ActiveRecord::Base.connection_pool.with_connection do
        begin
          run_unlocked
        ensure
          lock_file.flock(File::LOCK_UN) rescue nil
          lock_file.close rescue nil
        end
      end
    end

    :started
  end

  def self.run
    lock_file = File.open(LOCK_PATH, "w")
    unless lock_file.flock(File::LOCK_EX | File::LOCK_NB)
      lock_file.close
      raise "Game data scrape is already running"
    end

    begin
      run_unlocked
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

  def self.scrape_phase(phase, rounds: nil)
    puts "-> Scraping phase ##{phase.id} (#{phase.name}) url=#{phase.scrape_url} rounds=#{rounds.inspect}"
    options = {}
    options[:rounds] = rounds if rounds
    scrape(phase.id, phase.scrape_url, options)
    puts "   done phase ##{phase.id}"
  rescue => e
    puts "   ERROR phase ##{phase.id}: #{e.class}: #{e.message}"
  end

  def self.scrape_phase_async(phase, rounds: nil)
    Thread.new do
      ActiveRecord::Base.connection_pool.with_connection do
        scrape_phase(phase, rounds: rounds)
      end
    end
    :started
  end

  def self.run_unlocked
    phases = phases_to_scrape
    puts "Found #{phases.size} phase(s) with pending games to scrape"

    queue = Queue.new
    phases.each { |phase| queue << phase }

    workers = [MAX_CONCURRENCY, phases.size].min.times.map do
      Thread.new do
        ActiveRecord::Base.connection_pool.with_connection do
          while (phase = queue.pop(true) rescue nil)
            scrape_phase(phase)
          end
        end
      end
    end

    workers.each(&:join)
    puts "Game data scrape finished"
  end
end
