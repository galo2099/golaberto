class UserController < ApplicationController
  load_and_authorize_resource

  def show
    @comment_number = @user.comments.count
    @edit_number = @user.game_edits.count
  end

  def list_edits
    @edits = @user.game_edits.where("version > 1").paginate(:page => params[:page]).order("updated_at DESC")
  end

  def list
    # We do pagination by hand because we can't count the users with the custom select
    page = (params[:page] || 1).to_i
    per_page = 10
    offset = (page - 1) * per_page
    @users = User.select("*, (select count(game_versions.id) from game_versions where game_versions.updater_id = users.id) edit_count, " +
                   "(select count(comments.id) from comments where `comments`.user_id = users.id) comment_count").
        order("edit_count DESC, comment_count DESC, last_login DESC, name, login, identity_url").offset(offset).limit(per_page)
    @user_pagination = WillPaginate::Collection.new(page, per_page, User.count)
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
