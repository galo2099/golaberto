class PlayerController < ApplicationController
  scaffold :player

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
    team_player.destroy
    render :nothing => true
  end

  def list
    items_per_page = 10
    conditions = ["name LIKE ?", "%#{@params[:id]}%"] unless @params[:id].nil?

    @total = Player.count :conditions => conditions
    @player_pages, @players = paginate :players, :order => "name",
                                       :conditions => conditions,
                                       :per_page => items_per_page

    if request.xhr?
      render :partial => "player_list", :layout => false
    end
  end

  def show
    @player = Player.find(@params["id"])
  end

  def edit
    @player = Player.find(@params["id"])
  end
end
