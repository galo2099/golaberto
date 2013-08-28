class Referee < ActiveRecord::Base
  has_many :comments, ->{ order(created_at: :asc) }, :as => :commentable, :dependent => :destroy
  has_many :games, :dependent => :nullify
  has_many :played_games,
	   ->{ where(played: true).order(date: :desc) },
           :class_name => "Game"
  validates_presence_of :name
  validates_length_of :name, :within => 1..255
  validates_uniqueness_of :name

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: name , SQL Definition:varchar(255)
  # Field: location , SQL Definition:varchar(255)

  def to_param
    "#{id}-#{name.parameterize}"
  end
end
