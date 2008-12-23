class UserController < ApplicationController
  before_filter :editing_himself?, :only => [ :edit, :update ]

  def show
    @user = User.find(params[:id])
    @comment_number = @user.comments.count
    @last_comments = @user.comments.find(:all, :order => "created_at DESC", :limit => 5)
    @edit_number = @user.game_edits.count
    @last_edits = @user.game_edits.find(:all, :order => "updated_at DESC", :limit => 5)
  end

  def list
    @users = User.paginate :order => "name, login, identity_url",
                           :page => params[:page]
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
