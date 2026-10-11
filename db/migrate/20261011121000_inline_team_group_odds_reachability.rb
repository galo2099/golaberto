class InlineTeamGroupOddsReachability < ActiveRecord::Migration[8.1]
  class TeamGroup < ActiveRecord::Base
    self.table_name = "team_groups"
    serialize :odds
    serialize :odds_reachability
  end

  def up
    return unless column_exists?(:team_groups, :odds_reachability)

    TeamGroup.reset_column_information
    TeamGroup.find_each(batch_size: 1000) do |team_group|
      team_group.with_lock do
        compact_odds = compact(team_group.odds, team_group.odds_reachability)
        next if team_group.odds == compact_odds && team_group.odds_reachability.nil?

        team_group.update_columns(odds: compact_odds, odds_reachability: nil)
      end
    end

    remove_column :team_groups, :odds_reachability
    TeamGroup.reset_column_information
  end

  def down
    add_column :team_groups, :odds_reachability, :text unless column_exists?(:team_groups, :odds_reachability)
    TeamGroup.reset_column_information

    TeamGroup.find_each(batch_size: 1000) do |team_group|
      team_group.with_lock do
        if team_group.odds.nil?
          numeric_odds = nil
          statuses = nil
        else
          numeric_odds = team_group.odds.map { |entry| entry.is_a?(Array) ? entry.first : entry }
          statuses = team_group.odds.each_with_index.map do |entry, index|
            value = entry.is_a?(Array) ? entry.first : entry
            inline_status = entry.is_a?(Array) ? entry[1] : nil
            existing_status = team_group.odds_reachability && team_group.odds_reachability[index]
            if %w[reachable impossible undecided].include?(inline_status)
              inline_status
            elsif %w[reachable impossible undecided].include?(existing_status)
              existing_status
            elsif value.is_a?(Numeric) && value > 0
              "reachable"
            else
              "undecided"
            end
          end
        end

        team_group.update_columns(odds: numeric_odds, odds_reachability: statuses)
      end
    end
  end

  private

  def compact(odds, statuses)
    return nil if odds.nil?

    odds.each_with_index.map do |entry, index|
      value = entry.is_a?(Array) ? entry.first : entry
      inline_status = entry.is_a?(Array) ? entry[1] : nil
      status = if %w[reachable impossible undecided].include?(inline_status)
                 inline_status
               else
                 statuses && statuses[index]
               end

      if value.is_a?(Numeric) && value.zero? && %w[reachable impossible].include?(status)
        [value, status]
      else
        value
      end
    end
  end
end
