class TeamGroup < ApplicationRecord
  serialize :odds
  serialize :odds_reachability
  belongs_to :group, :touch => true
  belongs_to :team
  has_many :odds_histories, class_name: "TeamGroupOddsHistory", dependent: :delete_all
  validates_numericality_of :add_sub, :only_integer => true
  validates_numericality_of :bias, :only_integer => true
  validates_uniqueness_of :team_id, :scope => :group_id

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: group_id , SQL Definition:bigint(20)
  # Field: team_id , SQL Definition:bigint(20)
  # Field: add_sub , SQL Definition:int(4)
  # Field: bias , SQL Definition:tinyint(4)
  # Field: comment , SQL Definition:text

  def calculate_odds(positions)
    self.class.calculate_odds_for(odds, positions, odds_reachability)
  end

  def self.calculate_odds_for(odds, positions, statuses = nil)
    return nil if odds.nil? || odds.empty? || positions.nil? || positions.empty?

    value = positions.sum{|p| odds[p-1].to_f}
    if odds.all?{|probability| probability.is_a?(Numeric)} &&
       positions.uniq.size == positions.size &&
       odds.each_with_index.all?{|probability, index| positions.include?(index + 1) || (probability == 0 && statuses && statuses[index] == "impossible")} &&
       # The odds service allows this much row-sum error when balancing rare estimates.
       (value - 100).abs <= 0.0001
      return 100.0
    end

    value
  end

  def odds_reachability_for(positions)
    self.class.reachability_for(odds, odds_reachability, positions)
  end

  def self.reachability_for(odds, statuses, positions)
    return "undecided" if odds.nil? || positions.nil? || positions.empty?
    return "reachable" if positions.any?{|p| odds[p-1].to_f > 0}

    selected = positions.map{|p| statuses && statuses[p-1]}
    return "impossible" if selected.all?{|status| status == "impossible"}
    return "reachable" if selected.include?("reachable")

    "undecided"
  end

  # Store only the API's three public states, aligned with the numeric odds.
  # Older services omit metadata; clear it rather than keeping stale proofs.
  def assign_calculated_odds!(response)
    self.odds = response.fetch("team_odds").fetch(team_id.to_s).fetch("Pos")
    cells = (response["rare_position_estimates"] || {})[team_id.to_s]
    self.odds_reachability = cells && odds.each_index.map do |index|
      status = cells.fetch(index.to_s, {})["reachability"]
      %w[impossible reachable undecided].include?(status) ? status : "undecided"
    end
  end

  def record_odds_snapshot!(captured_at = Time.zone.now)
    return if odds.nil?

    snapshot = odds_histories.find_or_initialize_by(recorded_on: captured_at.to_date)
    snapshot.odds = odds
    snapshot.captured_at = captured_at
    snapshot.save!
  end
end
