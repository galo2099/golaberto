namespace :game_scrape do
  desc "Scrape game data for phases with pending games. Usage: rake game_scrape:run"
  task run: :environment do
    GameDataScrapeService.run
  end

  desc "Start game data scraping in a background thread (non-blocking). Usage: rake game_scrape:start_async"
  task start_async: :environment do
    result = GameDataScrapeService.start_async
    case result
    when :started
      puts "Game data scrape started in background"
    when :busy
      puts "Game data scrape is already running"
    end
  end
end
