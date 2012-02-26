require 'diff/lcs.rb'
class Game < ActiveRecord::Base

  # This module implements a diff with knowledge of associations
  module GameDiff
    def self.included(base)
      base.diff :include => [ "referee_id", "stadium_id" ],
                :exclude => [ "version", "updated_at" ]
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
                    :as => :commentable,
                    :dependent => :destroy,
                    :order => 'created_at ASC'
      base.belongs_to :home, :class_name => "Team", :foreign_key => "home_id"
      base.belongs_to :away, :class_name => "Team", :foreign_key => "away_id"
      base.belongs_to :phase
      base.belongs_to :stadium
      base.belongs_to :referee

      base.belongs_to :updated_by,
                      :class_name => "User",
                      :foreign_key => "updater_id"

      base.has_many :home_goals,
                    :class_name => "Goal",
                    :order => :time,
                    :include => :player,
                    :conditions => '(team_id = #{home_id} and own_goal = "0") or (team_id = #{away_id} and own_goal = "1")'

      base.has_many :away_goals,
                    :class_name => "Goal",
                    :order => :time,
                    :include => :player,
                    :conditions => '(team_id = #{away_id} and own_goal = "0") or (team_id = #{home_id} and own_goal = "1")'

      base.has_many :player_games,
                    :include => :player,
                    :dependent => :delete_all

      base.has_many :home_player_games,
                    :class_name => "PlayerGame",
                    :conditions => 'team_id = #{home_id}',
                    :include => :player

      base.has_many :away_player_games,
                    :class_name => "PlayerGame",
                    :conditions => 'team_id = #{away_id}',
                    :include => :player

      base.validates_presence_of :home
      base.validates_presence_of :away
      base.validates_presence_of :phase
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

      base.scope :group_games, lambda { |g|
        where("(home_id in (?) OR away_id in (?))", g.teams, g.teams)
      }

      base.scope :team_games, lambda { |t|
        where("(home_id = ? or away_id = ?)", t, t)
      }

    end
  end

  # This helper methods are also shared between the main class and the
  # versioned class
  module GameMethods
    def validate
      errors.add(:home, "can't play with itself") if home_id == away_id
    end

    def played_str
      played? ? _("Played") : _("Scheduled")
    end

    def formatted_date(day = false)
      ret = ""
      unless date.nil?
        ret = date.strftime("%d/%m/%Y")
        ret << " - " << date.strftime("%A") if day
      end
      ret
    end

    def formatted_time
      unless time.nil?
        time.strftime("%H:%M")
      end
    end

    def goal_distribution(team, side, score)
      phase.championship.games.find(:all, :order => :date).select{|g| g.played? and g.send(side) == team and g.date < date}.map{|g| g.send(score)}.inject(Array.new(10, 0)) {|a,x| a.map!{|z|z*0.8}; a[[x, 9].min]+=1;a}
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

    def odds
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
      end
    end
  end

  # The acts_as_versioned :extend option also includes the module in the main
  # class
  acts_as_versioned :extend => GameHelpers,
                    :if_changed => [ :round, :attendance, :date, :time,
                                     :stadium_id, :referee_id, :home_score,
                                     :away_score, :home_pen, :away_pen,
                                     :home_aet, :away_aet, :played ]

  # The versioned association is not shared because it is added automatically
  # to the versioned class by the version_association method
  has_many :goals,
           :order => :time,
           :include => :player
  version_association :goals

  # Always save the version. We check if it the game has really changed before saving it.
  def save_version?
    true
  end

  def find_n_previous_games_by_team_versus_team(n)
    Game.find :all, :limit => n, :include => { :phase => :championship }, :order => "date desc",
               :conditions => [ "((home_id = ? and away_id = ?) or (home_id = ? and away_id = ?)) and played = ? and championships.category_id = ? and date < ?",
                                self.home, self.away, self.away, self.home, true, self.phase.championship.category, self.date ]
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
  # Field: time , SQL Definition:time
  # Field: stadium_id , SQL Definition:bigint(20)
  # Field: referee_id , SQL Definition:bigint(20)
  # Field: home_score , SQL Definition:tinyint(2)
  # Field: away_score , SQL Definition:tinyint(2)
  # Field: home_pen , SQL Definition:tinyint(2)
  # Field: away_pen , SQL Definition:tinyint(2)
  # Field: played , SQL Definition:tinyint(1)

end
