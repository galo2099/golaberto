class HomeController < ApplicationController
  N_("Home")

  def index
    @today = Date.today
    @championships = Championship.find :all,
        :conditions => [ "begin <= ? AND end >= ?", @today, @today ],
        :order => "category_id, begin"
    @comments = Comment.find :all, :order => "created_at DESC", :limit => 5
    @games = Game.find :all, :order => [ "updated_at DESC" ], :limit => 5
    @graph = open_flash_chart_object(500, 250, '/home/graph')
  end

  def graph
    max = 20
    tmp = []
    10.times do |x|
      tmp << rand(max)
    end

    title = Title.new("MY TITLE")
    bar = Line.new
    bar.append_value({ :value => 3})
    bar.append_value({ :value => 6, :link => "http://www.google.com" })
    chart = OpenFlashChart.new
    chart.set_title(title)
    chart.add_element(bar)
    render :text => chart.to_s
  end
end
