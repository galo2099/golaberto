class CommentController < ApplicationController
  before_filter :login_required
  
  def new_game
    game = Game.find(params[:id])
    new(game)
  end

  private

  def new(object)
    @comment = Comment.new(params[:comment])
    @comment.created_at = Time.now
    @comment.user = current_user
    object.comments << @comment
    render :action => :new, :layout => false
  end
end
