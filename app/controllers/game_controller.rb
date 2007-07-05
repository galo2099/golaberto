class GameController < ApplicationController
  scaffold :game

  def show
    @game = Game.find(@params["id"])
  end

end
