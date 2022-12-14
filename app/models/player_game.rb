class PlayerGame < ApplicationRecord
  belongs_to :player
  belongs_to :game
  belongs_to :team

  validates_inclusion_of :yellow, :in => [ true, false ]
  validates_inclusion_of :red, :in => [ true, false ]

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: player_id , SQL Definition:bigint(20)
  # Field: game_id , SQL Definition:bigint(20)
  # Field: team_id , SQL Definition:bigint(20)
  # Field: on , SQL Definition:tinyint(2)
  # Field: off , SQL Definition:tinyint(2)
  # Field: yellow , SQL Definition:boolean
  # Field: red , SQL Definition:boolean

  def <=>(another)
    if (on == 0 and off == 0)
      if another.on == 0 and another.off == 0
        0
      else
        1
      end
    elsif another.on == 0 and another.off == 0
      -1
    elsif on != another.on
      on <=> another.on
    else
      Player.compare_position(player.position, another.player.position)
    end
  end
end
