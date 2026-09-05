class Championship < ApplicationRecord
  enum :region, [ :world, :continental, :national ]
  N_("World")
  N_("Continental")
  N_("National")

  has_many :phases, ->{ order :order_by }, :dependent => :destroy
  has_many :games, ->{ order :date }, :through => :phases
  has_many :goals, :through => :games
  has_many :team_players, :dependent => :delete_all
  has_many :teams, ->{ distinct }, :through => :phases
  belongs_to :category
  validates_presence_of :name
  validates_presence_of :begin
  validates_presence_of :end
  validates_numericality_of :point_win, :only_integer => true
  validates_numericality_of :point_draw, :only_integer => true
  validates_numericality_of :point_loss, :only_integer => true
  
  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: name , SQL Definition:varchar(255)
  # Field: begin , SQL Definition:date
  # Field: end , SQL Definition:date
  # Field: point_win , SQL Definition:tinyint(4)
  # Field: point_draw , SQL Definition:tinyint(4)
  # Field: point_loss , SQL Definition:tinyint(4)

  def self.regions_for_select
    self.regions.map do |name, id|
      [ _(name.capitalize), name ]
    end
  end

  def to_param
    "#{id}-#{full_name.parameterize}"
  end

  def season
    s = self.begin.year.to_s
    if self.begin.year != self.end.year
      s += "/" + self.end.year.to_s
    end
    s
  end

  def full_name(include_region = true, include_season = true)
    region = if include_region then "#{_(self.region_name)} - " else "" end
    season_suffix = if include_season then
      " #{self.begin.year.to_s}" + (if self.begin.year != self.end.year then "/#{self.end.year}" else "" end)
    else
      ""
    end
    name = "#{region}#{self.name}#{season_suffix}"

    if self.category_id != Category::DEFAULT_CATEGORY
      name += " - " + self.category.name
    end
    name
  end

  def avg_team_rating
    @avg_team_rating ||= begin teams.sort_by{|t|-t.rating.to_f}.each_with_index.map{|t,i|t.rating.to_f * (0.7)**i}.sum / teams.each_with_index.map{|t,i| 0.7**i}.sum rescue 0 end
  end

  def clone_phase_to_new_championship!(phase)
    raise ActiveRecord::RecordNotFound if phase.championship_id != id

    ActiveRecord::Base.transaction do
      cloned_championship = Championship.create!(
        name: "#{name} - #{phase.name}",
        begin: self.begin,
        end: self.end,
        point_win: point_win,
        point_draw: point_draw,
        point_loss: point_loss,
        category_id: category_id,
        show_country: show_country,
        region: region,
        region_name: region_name
      )

      cloned_phase = cloned_championship.phases.create!(
        name: phase.name,
        order_by: 1,
        sort: phase.sort,
        bonus_points: phase.bonus_points,
        bonus_points_threshold: phase.bonus_points_threshold,
        scrape_url: phase.scrape_url,
        sofascore_tournament_ids: phase.sofascore_tournament_ids
      )

      phase.groups.includes(:team_groups).order(:id).each do |group|
        cloned_group = cloned_phase.groups.create!(
          name: group.name,
          zones: group.zones
        )

        group.team_groups.each do |team_group|
          cloned_group.team_groups.create!(
            team_id: team_group.team_id,
            add_sub: team_group.add_sub,
            bias: team_group.bias,
            comment: team_group.comment
          )
        end
      end

      cloned_championship
    end
  end
end
