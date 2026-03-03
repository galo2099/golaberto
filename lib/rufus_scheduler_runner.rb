class RufusSchedulerRunner
  def self.start
    new.start
  end

  def start
    scheduler.every '1h', first_in: '0s' do
      refetch = Time.current.hour == 3
      include_all_past_unplayed = Time.current.hour == 3
      Rails.logger.info "[scheduler] Starting periodic game data scrape (refetch=#{refetch}, include_all_past_unplayed=#{include_all_past_unplayed})"
      result = GameDataScrapeService.start_async(refetch: refetch, include_all_past_unplayed: include_all_past_unplayed)
      Rails.logger.info "[scheduler] Game data scrape result: #{result}"
    end

    scheduler.cron '30 4 * * *' do
      Rails.logger.info '[scheduler] Starting daily player ratings update'
      begin
        response = PlayerRatingUpdateService.run
        Rails.logger.info "[scheduler] Player ratings update finished with status #{response.code}"
      rescue => e
        Rails.logger.error "[scheduler] ERROR updating player ratings: #{e.class}: #{e.message}"
      end
    end

    scheduler.join
  end

  private

  def scheduler
    @scheduler ||= Rufus::Scheduler.new
  end
end
