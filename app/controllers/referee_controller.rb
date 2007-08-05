class RefereeController < ApplicationController
  scaffold :referee
  before_filter :login_required, :except => [ :index, :list, :show ]

  def index
    redirect_to :action => :list
  end

  def list
    items_per_page = 10
    @name = params[:name]
    conditions = ["name LIKE ?", "%#{@name}%"] unless @name.nil?

    @total = Referee.count :conditions => conditions
    @referee_pages, @referees = paginate(:referees, :order => "name",
                                         :conditions => conditions,
                                         :per_page => items_per_page)

    if request.xhr?
      render :partial => "referee_list", :layout => false
    end
  end

  def show
    store_location
    @referee = Referee.find(params[:id])
  end
end
