class HomeController < ApplicationController
  N_("Home")

  def index
    @today = Date.today
    @championships = Championship.find :all,
        :conditions => [ "begin <= ? AND end >= ?", @today+30, @today-30 ],
        :order => "category_id, name, begin"
    @comments = Comment.find :all, :order => "created_at DESC", :limit => 5
    @games = Game.find :all, :conditions => "updater_id != 0", :order => [ "updated_at DESC" ], :limit => 5
  end
end
