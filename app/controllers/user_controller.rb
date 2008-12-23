class UserController < ApplicationController
  before_filter :editing_himself?, :only => [ :edit, :update ]

  def show
    @user = User.find(params[:id])
    @comment_number = @user.comments.count
    @edit_number = @user.game_edits.count
  end

  def list
    @users = User.paginate(
        :select => "*, (select count(game_versions.id) from game_versions where game_versions.updater_id = users.id) edit_count, " +
                   "(select count(comments.id) from comments where `comments`.user_id = users.id) comment_count",
        :order => "edit_count DESC, comment_count DESC, name, login, identity_url",
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
    flash[:notice] = _("Your profile was saved")
    redirect_to :action => :show, :id => @user
  rescue ActiveRecord::RecordInvalid
    render :action => :edit
  end

  private
    def editing_himself?
      return access_denied unless User.find(params[:id]) == current_user
      true
    end
end
