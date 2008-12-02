class HomeController < ApplicationController
  N_("Home")

  def index
    @today = Date.today
    @championships = Championship.find :all,
        :conditions => [ "begin <= ? AND end >= ?", @today, @today ],
        :order => "category_id, begin"
    @comments = Comment.find :all, :order => "created_at DESC", :limit => 5
    @games = Game.find :all, :order => [ "updated_at DESC" ], :limit => 5
  end
end
