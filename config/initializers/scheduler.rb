require 'rufus-scheduler'

return if defined?(Rails::Console) || Rails.env.test? || File.basename($0) == "rake"

scheduler = Rufus::Scheduler.singleton

scheduler.every '1h', first_in: '5m' do
  refetch = Time.current.hour == 3
  Rails.logger.info "[scheduler] Starting periodic game data scrape (refetch=#{refetch})"
  result = GameDataScrapeService.start_async(refetch: refetch)
  Rails.logger.info "[scheduler] Game data scrape result: #{result}"
end
