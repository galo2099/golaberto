class Phase < ActiveRecord::Base
  belongs_to :championship
  has_many   :groups, :dependent => :destroy, :order => :id
  has_many   :games, :dependent => :destroy
  validates_length_of :name, :within => 1..40
  validates_uniqueness_of :name, :scope => :championship_id
  validates_numericality_of :order_by, :only_integer => true
  validates_presence_of :sort
  validates_presence_of :championship, :message => "doesn't exist"
  validates_uniqueness_of :order_by, :scope => :championship_id

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: championship_id , SQL Definition:bigint(20)
  # Field: name , SQL Definition:varchar(255)
  # Field: order_by , SQL Definition:tinyint(4)
  # Field: sort , SQL Definition:varchar(255)

  def teams
    ret = Array.new
    groups.each do |group|
      ret.concat group.teams
    end
    ret.sort do |a,b| a.name <=> b.name end
  end

end
