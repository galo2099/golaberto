class UserController < ApplicationController
  authorize_resource

  def show
    @user = User.find(params[:id])
    @comment_number = @user.comments.count
    @edit_number = @user.game_edits.count
  end

  def list_edits
    @user = User.find(params[:id])
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
    @user = User.find(params[:id])
  end

  def update
    @user = User.find(params[:id])
    birthday = params[:user].delete(:birthday)
    @user.birthday = Date.strptime(birthday, "%d/%m/%Y") unless birthday.empty?
    @user.attributes = params[:user]
    @user.save!
    @user.uploaded_picture(params[:picture], params[:filter]) unless params[:picture].to_s.empty?
    flash[:notice] = _("Your profile was saved")
    redirect_to :action => :show, :id => @user
  rescue ActiveRecord::RecordInvalid
    render :action => :edit
  end
end
