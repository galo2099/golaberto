require 'scrape'

class GameDataScrapeService
  LOCK_PATH = Rails.root.join("tmp", "game_data_scrape.lock")

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

  def self.run_unlocked
    phases = phases_to_scrape
    puts "Found #{phases.size} phase(s) with pending games to scrape"

    phases.each do |phase|
      puts "-> Scraping phase ##{phase.id} (#{phase.name}) url=#{phase.scrape_url}"
      begin
        scrape(phase.id, phase.scrape_url)
        puts "   done"
      rescue => e
        puts "   ERROR: #{e.class}: #{e.message}"
      end
    end

    puts "Game data scrape finished"
  end
end
