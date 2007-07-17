class Player < ActiveRecord::Base
  has_many :goals, :dependent => :destroy
  has_many :team_players, :dependent => :delete_all

  validates_length_of :name, :within => 1..40

  Positions = %w(g dr dl dc dm cm am fw)
  validates_inclusion_of :position, :in => Positions, :allow_nil => true

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: name , SQL Definition:varchar(255)
  # Field: position , SQL Definition:tinytext
  # Field: birth , SQL Definition:date
  # Field: country , SQL Definition:varchar(255)
  # Field: full_name , SQL Definition:varchar(255)


end
