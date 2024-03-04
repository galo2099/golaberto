require 'diff/lcs.rb'
require 'active_record/diff'
require 'poisson'
class Game < ApplicationRecord
  AVG_BASE = 1.3350257653834494
  HOME_ADV = 0.16133676871779334

  # This module implements a diff with knowledge of associations
  module GameDiff
    def self.included(base)
      base.diff :include => [ :referee_id, :stadium_id ],
                :exclude => [ :version, :updated_at ]
      base.class_eval do
        alias_method :diff_without_associations, :diff
        alias_method :diff, :diff_with_association
      end
    end

    def diff_with_association(model = self.class.find(id))
      model_diff = diff_without_associations(model)
      g1 = goals.map do |goal|
        ret = Goal.content_columns.map do |c|
          [ c.name,  goal.send(c.name) ]
        end
        ret << [ "player_id", goal.player_id ]
      end
      g2 = model.goals.map do |goal|
        ret = Goal.content_columns.map do |c|
          [ c.name, goal.send(c.name) ]
        end
        ret << [ "player_id", goal.player_id ]
      end
      goal_diff = Diff::LCS.diff(g1, g2)
      model_diff.merge!({:goals => goal_diff}) unless goal_diff.empty?
      model_diff
    end
  end

  # This module includes all non-versioned associations and is shared by Game
  # and Game::Version, created by acts_as_versioned
  module GameAssociations
    def self.included(base)
      base.has_many :comments,
	            ->{ order(created_at: :asc) },
                    :as => :commentable,
                    :dependent => :destroy
      base.belongs_to :home, :class_name => "Team", :foreign_key => "home_id"
      base.belongs_to :away, :class_name => "Team", :foreign_key => "away_id"
      base.belongs_to :phase, :touch => true
      base.has_one :championship, through: :phase
      base.belongs_to :stadium, optional: true
      base.belongs_to :referee, optional: true

      base.has_many :player_games,
                    ->{ includes :player },
                    :dependent => :delete_all

      base.has_one :home_rating, ->(game){where("measure_date < ?", game.date).order(measure_date: :desc)}, through: :home, source: :historical_ratings
      base.has_one :away_rating, ->(game){where("measure_date < ?", game.date).order(measure_date: :desc)}, through: :away, source: :historical_ratings

      base.validates_presence_of :date
      base.validates_inclusion_of :played, :in => [ true, false ]
      base.validates_numericality_of :home_score, :only_integer => true
      base.validates_numericality_of :away_score, :only_integer => true
      base.validates_numericality_of :home_pen,
                                     :only_integer => true,
                                     :allow_nil => true
      base.validates_numericality_of :away_pen,
                                     :only_integer => true,
                                     :allow_nil => true
      base.validates_numericality_of :home_aet,
                                     :only_integer => true,
                                     :allow_nil => true
      base.validates_numericality_of :away_aet,
                                     :only_integer => true,
                                     :allow_nil => true

      base.scope :team_games, lambda { |t|
        base.where("(home_id = ? or away_id = ?)", t, t)
      }
    end
  end

  # This helper methods are also shared between the main class and the
  # versioned class
  module GameMethods
    def self.included(base)
      base.enum home_field: [ :left, :neutral, :right ]
      base.class_eval do
        def self.i18n_home_fields
          hash = {}
          home_fields.keys.each { |key| hash[I18n.t("activerecord.attributes.game.home_field.#{key}")] = key }
          hash
        end
      end
    end

    def home_goals
      goals.where('(team_id = ? and own_goal = 0) or (team_id = ? and own_goal = 1)', home_id, away_id)
    end

    def away_goals
      goals.where('(team_id = ? and own_goal = 0) or (team_id = ? and own_goal = 1)', away_id, home_id)
    end

    def home_player_games
      player_games.where(:team_id => home_id)
    end

    def away_player_games
      player_games.where(:team_id => away_id)
    end

    def validate
      errors.add(:home, _("can't play with itself")) if home_id == away_id
    end

    def played_str
      played? ? _("Played") : _("Scheduled")
    end

    def goal_distribution(team, side, score)
      phase.championship.games.order(:date)
          .select{|g| g.played? and g.send(side) == team and g.date < date}
          .map{|g| g.send(score)}
          .inject(Array.new(10) {|i| i < 3 ? 5 : 0}) {|a,x| a.map!{|z|z*0.8}; a[[x, 9].min]+=1;a}
    end

    def home_for
      goal_distribution(home_id, :home_id, :home_score)
    end

    def home_against
      goal_distribution(home_id, :home_id, :away_score)
    end

    def away_for
      goal_distribution(away_id, :away_id, :away_score)
    end

    def away_against
      goal_distribution(away_id, :away_id, :home_score)
    end

    def left_advantage
      case home_field
      when "left"
        HOME_ADV
      when "neutral"
        0.0
      when "right"
        -HOME_ADV
      end
    end

    def home_power
      if home_rating.nil? or away_rating.nil?
        return nil
      end
      [10.0, [0.01, (home_rating.off_rating.to_f - AVG_BASE)/(AVG_BASE*0.424+0.548)*([0.25, (away_rating.def_rating.to_f+left_advantage)*0.424+0.548].max)+(away_rating.def_rating.to_f+left_advantage)].max].min
    end

    def away_power
      if home_rating.nil? or away_rating.nil?
        return nil
      end
      [10.0, [0.01, (away_rating.off_rating.to_f - AVG_BASE)/(AVG_BASE*0.424+0.548)*([0.25, (home_rating.def_rating.to_f-left_advantage)*0.424+0.548].max)+(home_rating.def_rating.to_f-left_advantage)].max].min
    end

    def odds
      goal_array = (0...20).to_a
      h = home_power
      a = away_power
      if h.nil? or a.nil?
        return nil
      end
      hp = Poisson.new(h)
      ap = Poisson.new(a)
      probs = goal_array.map{|i| goal_array.map{|j| hp.p(i) * ap.p(j) }}
      [ goal_array.map{|i| (0...i).to_a.map{|j| probs[i][j]}}.flatten.sum,
        goal_array.map{|i| probs[i][i]}.sum,
        goal_array.map{|i| (0...i).to_a.map{|j| probs[j][i]}}.flatten.sum ]
    end

    def game_quality
      # add EPSILON to avoid division by zero
      2 * home.rating.to_f * away.rating.to_f / (home.rating.to_f + away.rating.to_f + Float::EPSILON) * (1 + (home_importance.to_f + away_importance.to_f) / 2)
    end

    def odds_legacy
      #home_power = (mean_poisson(home_for) + mean_poisson(away_against)) / 2
      #away_power = (mean_poisson(home_against) + mean_poisson(away_for)) / 2
      home_power = Poisson.new(Poisson.find_mean_from(home_for.zip(away_against).map{|a,b| a+b}))
      away_power = Poisson.new(Poisson.find_mean_from(home_against.zip(away_for).map{|a,b| a+b}))

      ten_array = (0...10).to_a
      probs = ten_array.map{|i| ten_array.map{|j| home_power.p(i) * away_power.p(j) }}
      [ ten_array.map{|i| (0...i).to_a.map{|j| probs[i][j]}}.flatten.sum,
        ten_array.map{|i| probs[i][i]}.sum,
        ten_array.map{|i| (0...i).to_a.map{|j| probs[j][i]}}.flatten.sum ]
    end
  end

  # As acts_as_versioned only accepts one module to extend, this helper module
  # joins all the above modules
  module GameHelpers
    def self.included(base)
      base.class_eval do
        include ActiveRecord::Diff
        include GameDiff
        include GameAssociations
        include GameMethods
        stampable
      end
    end
  end

  # The acts_as_versioned :extend option also includes the module in the main
  # class
  acts_as_versioned :extend => GameHelpers,
                    :if_changed => [ :round, :attendance, :date, :has_time,
                                     :stadium_id, :referee_id, :home_score,
                                     :away_score, :home_pen, :away_pen,
                                     :home_aet, :away_aet, :played, :home_field ]

  # The versioned association is not shared because it is added automatically
  # to the versioned class by the version_association method
  has_many :goals,
	   ->{ order(:time) }
  version_association :goals

  # Always save the version. We check if it the game has really changed before saving it.
  def save_version?
    true
  end

  def find_n_previous_games_by_team_versus_team(n)
    Game.limit(n).includes(:phase => :championship).order("date desc").
        where("((home_id = ? and away_id = ?) or (home_id = ? and away_id = ?)) and played = ? and championships.category_id = ? and date < ?",
              self.home, self.away, self.away, self.home, true, self.phase.championship.category, self.date).references(:championship)
  end

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: home_id , SQL Definition:bigint(20)
  # Field: away_id , SQL Definition:bigint(20)
  # Field: phase_id , SQL Definition:bigint(20)
  # Field: round , SQL Definition:tinyint(4)
  # Field: attendance , SQL Definition:mediumint(9)
  # Field: date , SQL Definition:date
  # Field: has_time , SQL Definition:tinyint(1)
  # Field: stadium_id , SQL Definition:bigint(20)
  # Field: referee_id , SQL Definition:bigint(20)
  # Field: home_score , SQL Definition:tinyint(2)
  # Field: away_score , SQL Definition:tinyint(2)
  # Field: home_pen , SQL Definition:tinyint(2)
  # Field: away_pen , SQL Definition:tinyint(2)
  # Field: played , SQL Definition:tinyint(1)

end
