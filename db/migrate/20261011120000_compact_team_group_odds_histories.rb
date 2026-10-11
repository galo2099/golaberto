class CompactTeamGroupOddsHistories < ActiveRecord::Migration[8.1]
  class Snapshot < ActiveRecord::Base
    self.table_name = "team_group_odds_histories"
    serialize :odds
  end

  def up
    Snapshot.find_each(batch_size: 1000) do |snapshot|
      snapshot.with_lock do
        compact_odds = compact(snapshot.odds)
        next if snapshot.odds == compact_odds

        snapshot.update_columns(odds: compact_odds)
      end
    end
  end

  def down
    raise ActiveRecord::IrreversibleMigration, "Converting history back to numeric-only odds would discard reachability proofs."
  end

  private

  def compact(odds)
    return odds unless odds.is_a?(Array)

    odds.map do |entry|
      if entry.is_a?(Array)
        value = entry.first
        status = entry[1]
        if value.is_a?(Numeric) && value.zero?
          case status
          when "reachable", "impossible"
            [value, status]
          else
            value
          end
        elsif value.is_a?(Numeric)
          value
        else
          entry
        end
      else
        entry
      end
    end
  end
end
