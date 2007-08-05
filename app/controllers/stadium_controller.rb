class StadiumController < ApplicationController
  scaffold :stadium
  before_filter :login_required, :except => [ :index, :list, :show ]

  def index
    redirect_to :action => :list
  end

  def list
    items_per_page = 10
    @name = params[:name]
    conditions = ["name LIKE ?", "%#{@name}%"] unless @name.nil?

    @total = Stadium.count :conditions => conditions
    @stadium_pages, @stadia = paginate(:stadia, :order => "name",
                                       :conditions => conditions,
                                       :per_page => items_per_page)

    if request.xhr?
      render :partial => "stadium_list", :layout => false
    end
  end

  def show
    store_location
    @stadium = Stadium.find(params[:id])
  end
end
