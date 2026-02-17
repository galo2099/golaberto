class GroupController < ApplicationController
  N_("Group")

  skip_before_action :verify_authenticity_token, :only => [:team_list]
  authorize_resource

  def edit
    @group = Group.find(params["id"])
  end

  def team_list
    @teams = Team.order(:name)
    respond_to do |format|
      format.js
    end
  end

  def update
    @group = Group.find(params["id"])
    @group.update(group_params)
    @group.team_groups.each{|t|t.destroy}
    
    params["team_group"].each do |key, value|
      value = value.permit(:team_id, :add_sub, :bias, :comment)
      value["comment"] = nil if value["comment"].to_s.empty?
      @group.team_groups << TeamGroup.new(value.merge({:group_id => @group.id}))
    end unless params["team_group"].nil?

    @group.zones = []
    params["group"]["zones"].each do |value|
      value = value.permit(:name, :color, position: [])
      value[:position].map!{|p|p.to_i} if value[:position]
      @group.zones.push(value.to_hash)
    end unless params["group"]["zones"].nil?

    begin
      @group.save!
      redirect_to :controller => :championship, :action => :phases, :id => @group.phase.championship, :phase => @group.phase
    rescue
      @teams = Team.order(:name)
      render :action => "edit"
    end
  end

  def destroy
    group = Group.find(params["id"])
    group.destroy
    redirect_to :controller => :phase, :action => :edit, :id => group.phase
  end

  def odds_progress
    @group = Group.find(params["id"])
    if @group.odds_progress == 100 or @group.odds_progress.nil?
      @group.odds_progress = nil
      @group.save!
    end
  end

  def update_odds
    @group = Group.find(params["id"])
    if @group.odds_progress == nil
      @group.odds_progress = 0
      @group.save!
      snapshot_date = @group.games.where(played: true).maximum(:date)
      snapshot_time = if snapshot_date
                        snapshot_date.end_of_day
                      else
                        Time.zone.now
                      end
      Thread.new do
        ActiveRecord::Base.connection_pool.with_connection do
          @group.odds(snapshot_time: snapshot_time)
        end
      end
    end
    render :action => :odds_progress
  end


  def start_odds_history_backfill
    championship_id = params["id"]
    phase_id = params["phase"].presence
    from_date = params["from"].present? ? Date.parse(params["from"]) : nil
    to_date = params["to"].present? ? Date.parse(params["to"]) : nil
    @backfill_started = OddsHistoryBackfillService.start_async(
      championship_id: championship_id,
      phase_id: phase_id,
      from_date: from_date,
      to_date: to_date,
      reset: true,
    ) == :started
  rescue ArgumentError
    @backfill_started = false
    @backfill_invalid_params = true
  end

  private
  def group_params
    params.require(:group).permit(:name)
  end
end
