class Ability
  include CanCan::Ability

  def initialize(user)
    alias_action :list, :to => :read

    user ||= User.new # guest user (not logged in)
    if user.has_role?("editor")
      can :manage, :all
      cannot :manage, User
    end

    if user.has_role?("commenter")
      can :create, Comment
      can :destroy, Comment do |comment|
        comment.try(:user) == user
      end
    end

    can :read, :all
    can [ :phases, :crowd, :team, :games, :team_json, :top_goalscorers ], Championship
    can :games, Team
    can [ :create, :list_edits ], User
    can :update, User do |user_to_update|
      user_to_update == user
    end
  end
end
