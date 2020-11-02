class PlayerController < ApplicationController
  include ApplicationHelper

  N_("Player")

  authorize_resource

  def index
    list
    render :action => :list
  end

  def destroy_team
    team_player = TeamPlayer.find(params[:id])
    goals = team_player.player.goals.where(team_id: team_player.team_id)
    goals.each do |g|
      if g.game.phase.championship_id == team_player.championship_id
        g.destroy
      end
    end
    player_games = team_player.player.player_games.where(team_id: team_player.team_id)
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
    @country = params[:country]
    @continent = params[:continent] || ""
    @players = Player.order(:name)
    @players = @players.where(["name LIKE ?", "%#{@id}%"]) unless @id.nil?

    countries_with_players = { @country => 0 }
    countries_with_players.merge!(@players.group(:country).size)

    countries_found = []
    countries_not_found = []
    golaberto_options_for_country_select.each do |translated_country, original_country|
      count = countries_with_players[original_country]
      if count.nil? then
        countries_not_found << [translated_country, original_country]
      else
        countries_found << [translated_country + " (#{count})", original_country]
      end
    end

    @country_list = [[s_("Country|All") + " (#{@players.size})", ""]] + countries_found + countries_not_found
    @countries = {}
    @countries[""] = @country_list
    ApplicationHelper::Continent::ALL.each do |name, c|
      @countries[name] = [[s_("Country|All") + " (#{@players.where(country: c.countries.map{|c|c.name}).size})", ""]] + @country_list.select{|_, n| ApplicationHelper::Continent.country_to_continent[n] == c}
    end

    unless @continent.blank?
      @players = @players.where(country: Continent::ALL[@continent].countries.map{|c|c.name})
      unless Continent::ALL[@continent].countries.map{|c|c.name}.include? @country
        @country = nil
      end
    end

    @players = @players.page(params[:page])
    @players = @players.where(country: @country) unless @country.blank?
    puts @players.first
    if @players.size == 1
      redirect_to :action => :show, :id => @players.first
    end
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
    @player.attributes = player_params

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
    @player = Player.new(player_params)
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

  private

  def player_params
    params.require(:player).permit("name", "position", "birth", "country", "full_name")
  end
end
