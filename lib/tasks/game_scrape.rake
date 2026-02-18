namespace :game_scrape do
  desc "Scrape game data for phases with pending games. Usage: rake game_scrape:run [REFETCH=true]"
  task run: :environment do
    refetch = ENV["REFETCH"].to_s == "true"
    GameDataScrapeService.run(refetch: refetch)
  end

  desc "Start game data scraping in a background thread (non-blocking). Usage: rake game_scrape:start_async [REFETCH=true]"
  task start_async: :environment do
    refetch = ENV["REFETCH"].to_s == "true"
    result = GameDataScrapeService.start_async(refetch: refetch)
    case result
    when :started
      puts "Game data scrape started in background"
    when :busy
      puts "Game data scrape is already running"
    end
  end
end
