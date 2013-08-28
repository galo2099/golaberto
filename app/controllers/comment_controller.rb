class CommentController < ApplicationController
  N_("Comment")

  authorize_resource

  def new
    type = params[:type].classify.constantize
    object = type.find(params[:id])
    @comment = Comment.new(comment_params)
    @comment.created_at = Time.now
    @comment.user = current_user
    authorize! :create, @comment
    object.comments << @comment
    render :action => :new, :layout => false
  end

  def destroy
    @comment = Comment.find(params[:id])
    authorize! :destroy, @comment
    @comment.destroy
  end

  private
  def comment_params
    params.require(:comment).permit(:comment)
  end
end
