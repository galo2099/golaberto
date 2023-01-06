class TeamPlayer < ApplicationRecord
  belongs_to :team
  belongs_to :player
  belongs_to :championship

  validates_uniqueness_of :team_id, :scope => [ :player_id, :championship_id ]

  def self.stats(conditions)
    stats_by_game = PlayerGame.joins(:game).joins("LEFT JOIN `goals` ON `goals`.`game_id` = `games`.`id` and goals.player_id = player_games.player_id").group("player_games.id").select("player_games.id as id, SUM(IF(own_goal = 0, 1, 0)) as goals, SUM(IF(own_goal = 1, 1, 0)) as own_goals, SUM(IF(penalty = 1, 1, 0)) as penalties").where(conditions)

    PlayerGame.joins("INNER JOIN (#{stats_by_game.to_sql}) g1 on g1.id = player_games.id").joins(game: {phase: :championship}).group("phases.championship_id, team_id, player_id").select("championship_id, player_id, player_games.team_id, player_games.id, SUM(off-`on`) as minutes, SUM(yellow = 1) as yellows, SUM(red = 1) as reds, COUNT(*) as appearances, SUM(IF(off > 0, 1, 0)) as played, SUM(goals) as goals, SUM(own_goals) as own_goals, SUM(penalties) as penalties, SUM(IF(OFF = 0, 1, 0)) as bench, SUM(IF(`on` > 0, 1, 0)) as sub, MAX(date) as latest, player_games.game_id").order("latest DESC")
  end

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: team_id , SQL Definition:bigint(20)
  # Field: player_id , SQL Definition:bigint(20)
  # Field: championship_id , SQL Definition:bigint(20)
end
