class CommentController < ApplicationController
  N_("Comment")

  require_role "commenter"

  def new
    type = params[:type].classify.constantize
    object = type.find(params[:id])
    @comment = Comment.new(params[:comment])
    @comment.created_at = Time.now
    @comment.user = current_user
    object.comments << @comment
    render :action => :new, :layout => false
  end

  def destroy
    @comment = Comment.find(params[:id])
    if current_user == @comment.user
      @comment.destroy
    else
      render :nothing => true
    end
  end
end
