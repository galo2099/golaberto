class UserController < ApplicationController
  load_and_authorize_resource

  def show
    @comment_number = @user.comments.count
    @edit_number = @user.game_edits.count
  end

  def list_edits
    @pagy, @edits = pagy(@user.game_edits.where("version > 1").order("updated_at DESC"), items: 10)
  end

  def list
    @users = User.select("*, (select count(game_versions.id) from game_versions where game_versions.updater_id = users.id) edit_count, " +
                   "(select count(comments.id) from comments where `comments`.user_id = users.id) comment_count").
        order("edit_count DESC, comment_count DESC, last_login DESC, name, login, identity_url")
    @pagy, @users = pagy(@users, items: 10)
  end

  def edit
  end

  def update
    @user.attributes = user_params
    @user.save!
    flash[:notice] = _("Your profile was saved")
    redirect_to :action => :show, :id => @user
  rescue ActiveRecord::RecordInvalid
    render :action => :edit
  end

  private

  def user_params
    params.require(:user).permit("name", "email", "location", "birthday", "about_me", "filter_image_background", "avatar")
  end
end
