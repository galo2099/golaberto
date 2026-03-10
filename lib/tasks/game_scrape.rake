namespace :game_scrape do
  desc "Scrape game data for phases with pending games. Usage: rake game_scrape:run [REFETCH=true] [ALL_PAST_PENDING=true]"
  task run: :environment do
    refetch = ENV["REFETCH"].to_s == "true"
    include_all_past_unplayed = ENV["ALL_PAST_PENDING"].to_s == "true"
    GameDataScrapeService.run(refetch: refetch, include_all_past_unplayed: include_all_past_unplayed)
  end

  desc "Start game data scraping in a background thread (non-blocking). Usage: rake game_scrape:start_async [REFETCH=true] [ALL_PAST_PENDING=true]"
  task start_async: :environment do
    refetch = ENV["REFETCH"].to_s == "true"
    include_all_past_unplayed = ENV["ALL_PAST_PENDING"].to_s == "true"
    result = GameDataScrapeService.start_async(refetch: refetch, include_all_past_unplayed: include_all_past_unplayed)
    case result
    when :started
      puts "Game data scrape started in background"
    when :busy
      puts "Game data scrape is already running"
    end
  end
end
