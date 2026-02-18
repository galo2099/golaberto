class PhaseController < ApplicationController
  N_("Phase")

  authorize_resource

  def new
    @phase = Phase.new
  end

  def create
    @phase = Phase.new(phase_params)

    if @phase.save
      redirect_to :action => :show, :id => @phase
    else
      render :action => :new
    end
  end

  def edit
    @phase = Phase.find(params["id"])
  end

  def add_groups
    phase = Phase.find(params["id"])
    last_group = phase.groups[-1]
    count = params[:count].to_i
    if count >= 100
      raise "Can't add too many groups"
    end
    count.times do
      tokens = last_group.name.split(" ")
      tokens[-1].succ!
      new_name = tokens.join " "
      last_group = phase.groups.build
      last_group.name = new_name
      last_group.save!
    end
    render :js => "window.location = '#{url_for action: :edit, id: phase}'"
  end

  def update
    @phase = Phase.find(params["id"])
    @phase.attributes = phase_params

    saved = @phase.save
    new_empty = false

    @group = @phase.groups.build(group_params)
    new_empty = @group.name.empty?

    saved = @group.save unless new_empty

    if saved and new_empty
      redirect_to :controller => :championship, :action => :phases, :id => @phase.championship, :phase => @phase
    else
      render :action => "edit"
    end
  end

  def destroy
    phase = Phase.find(params["id"])
    phase.destroy
    redirect_to :controller => :championship, :action => :edit, :id => phase.championship
  end

  def start_scrape
    @phase = Phase.find(params["id"])
    if @phase.scrape_url.blank?
      @scrape_error = true
      return
    end

    rounds_param = params["rounds"].to_s.strip
    rounds = nil
    unless rounds_param.empty?
      rounds = rounds_param.split(/[\s,]+/).map(&:to_i).select { |r| r > 0 }.uniq.sort
      if rounds.empty?
        @scrape_invalid_rounds = true
        return
      end
    end

    GameDataScrapeService.scrape_phase_async(@phase, rounds: rounds)
    @scrape_started = true
  end

  private
  def phase_params
    params.require(:phase).permit(:name, :order_by, :sort, :bonus_points, :bonus_points_threshold, :scrape_url)
  end

  def group_params
    params.require(:group).permit(:name)
  end
end
