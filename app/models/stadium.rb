class Stadium < ActiveRecord::Base
  has_many :comments, :as => :commentable, :dependent => :destroy, :order => 'created_at ASC'
  has_many :games, :dependent => :nullify
  has_many :played_games,
           :class_name => "Game",
           :conditions => { :played => true },
           :order => "date DESC"
  validates_presence_of :name
  validates_length_of :name, :within => 1..255
  validates_uniqueness_of :name

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: name , SQL Definition:varchar(255)


end
