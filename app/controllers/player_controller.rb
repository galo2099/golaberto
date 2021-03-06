class PlayerController < ApplicationController
  N_("Player")

  require_role "editor", :except => [ :index, :list, :show ]

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

  def update
    @player = Player.find(params[:id])
    @player.attributes = params[:player]

    if @player.save
      flash[:notice] = _("Player was successfully updated")
      redirect_to :action => :show, :id => @player
    else
      render :action => :edit
    end
  end

  def new
    @player = Player.new
  end

  def create
    @player = Player.new(params[:player])
    if @player.save
      flash[:notice] = _("Player was successfully created")
      redirect_to :action => :show, :id => @player
    else
      render :action => :new
    end
  end

  def destroy
    Player.find(params[:id]).destroy
    redirect_to :action => :list
  end
end
