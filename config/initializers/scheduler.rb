require 'rufus-scheduler'

return if defined?(Rails::Console) || Rails.env.test? || File.basename($0) == "rake"

scheduler = Rufus::Scheduler.singleton

scheduler.every '1h', first_in: '5m' do
  Rails.logger.info "[scheduler] Starting periodic game data scrape"
  result = GameDataScrapeService.start_async
  Rails.logger.info "[scheduler] Game data scrape result: #{result}"
end

scheduler.cron '0 3 * * *' do
  Rails.logger.info "[scheduler] Starting daily full refetch scrape"
  result = GameDataScrapeService.start_async(refetch: true)
  Rails.logger.info "[scheduler] Daily full refetch result: #{result}"
end
