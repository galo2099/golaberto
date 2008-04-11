class PlayerController < ApplicationController
  scaffold :player
  before_filter :login_required, :except => [ :index, :list, :show ]

  def index
    list
    render :action => :list
  end

  def destroy_team
    team_player = TeamPlayer.find(params[:id])
    goals = team_player.player.goals.find_all_by_team_id(team_player.team_id)
    goals.each do |g|
      if g.game.phase.championship_id == team_player.championship_id
        g.destroy
      end
    end
    player_games = team_player.player.player_games.find_all_by_team_id(team_player.team_id)
    player_games.each do |pg|
      if pg.game.phase.championship_id == team_player.championship_id
        pg.destroy
      end
    end
    team_player.destroy
    render :nothing => true
  end

  def list
    @id = params[:id]
    conditions = ["name LIKE ?", "%#{@id}%"] unless @id.nil?

    @players = Player.paginate :order => "name",
                               :conditions => conditions,
                               :page => params[:page]
  end

  def show
    store_location
    @player = Player.find(params["id"])
  end

  def edit
    @player = Player.find(params["id"])
  end
end
