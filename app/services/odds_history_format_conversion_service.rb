class OddsHistoryFormatConversionService
  def self.run(batch_size: 1000)
    converted = 0

    TeamGroupOddsHistory.find_each(batch_size: batch_size) do |snapshot|
      snapshot.with_lock do
        canonical_odds = TeamGroupOddsHistory.encode_odds(snapshot.numeric_odds, snapshot.odds_reachability)
        next if snapshot.odds == canonical_odds

        snapshot.update_columns(odds: canonical_odds)
        converted += 1
      end
    end

    converted
  end
end
