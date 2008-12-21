class UserController < ApplicationController
  def show
    @user = User.find(params[:id])
    @comment_number = @user.comments.count
    @last_comments = @user.comments.find(:all, :order => "created_at DESC", :limit => 5)
    @edit_number = @user.game_edits.count
    @last_edits = @user.game_edits.find(:all, :order => "updated_at DESC", :limit => 5)
  end

  def edit
  end

  def list
    @users = User.paginate :order => "name, login, identity_url",
                           :page => params[:page]
  end

  def update
  end

end
