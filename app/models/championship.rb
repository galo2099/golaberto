class Championship < ActiveRecord::Base
  has_many :phases, :order => "order_by"

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: name , SQL Definition:varchar(255)
  # Field: begin , SQL Definition:date
  # Field: end , SQL Definition:date
  # Field: point_win , SQL Definition:tinyint(4)
  # Field: point_draw , SQL Definition:tinyint(4)
  # Field: point_loss , SQL Definition:tinyint(4)

  def full_name
    name = self.name + " " + self.begin.year.to_s
    if self.begin.year != self.end.year
      name += "/" + self.end.year.to_s
    end
    name
  end

end
