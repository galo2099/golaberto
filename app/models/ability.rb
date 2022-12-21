class Ability
  include CanCan::Ability

  def initialize(user)
    alias_action :list, :to => :read

    user ||= User.new # guest user (not logged in)
    if user.has_role?("editor")
      can :manage, :all
      cannot :manage, [ User, Comment ]
    end

    if user.has_role?("commenter")
      can :create, Comment
      can :destroy, Comment, :user_id => user.id
    end

    can :read, :all
    can [ :phases, :crowd, :team, :games, :team_json, :top_goalscorers ], Championship
    can [ :games, :historical_rating ], Team
    can [ :games ], Player
    can [ :create, :list_edits ], User
    can :update, User, :id => user.id
  end
end
