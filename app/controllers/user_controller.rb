class UserController < ApplicationController
  load_and_authorize_resource

  def show
    @comment_number = @user.comments.count
    @edit_number = @user.game_edits.count
  end

  def list_edits
    @edits = @user.game_edits.paginate(:page => params[:page], :order => "updated_at DESC", :conditions => [ "version > 1" ])
  end

  def list
    @users = User.paginate(
        :select => "*, (select count(game_versions.id) from game_versions where game_versions.updater_id = users.id) edit_count, " +
                   "(select count(comments.id) from comments where `comments`.user_id = users.id) comment_count",
        :order => "edit_count DESC, comment_count DESC, last_login DESC, name, login, identity_url",
        :page => params[:page])
  end

  def edit
  end

  def update
    @user.attributes = params[:user]
    @user.save!
    flash[:notice] = _("Your profile was saved")
    redirect_to :action => :show, :id => @user
  rescue ActiveRecord::RecordInvalid
    render :action => :edit
  end
end
