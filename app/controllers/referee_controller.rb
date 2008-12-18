class RefereeController < ApplicationController
  N_("Referee")

  scaffold :referee
  require_role "editor", :except => [ :index, :list, :show ]

  def index
    redirect_to :action => :list
  end

  def list
    @name = params[:name]
    conditions = ["name LIKE ?", "%#{@name}%"] unless @name.nil?

    @referees = Referee.paginate :order => "name",
                                 :conditions => conditions,
                                 :page => params[:page]
  end

  def show
    store_location
    @referee = Referee.find(params[:id])
  end
end
