class AccountController < ApplicationController
  N_("Account")

  skip_before_action :verify_authenticity_token, only: [:login]
  skip_authorization_check
  before_filter :login_from_cookie

  def login
    @user = User.new(params.fetch(:user, {}).permit(:login, :password))
    if using_open_id?
      open_id_authentication
    elsif not @user.login.blank?
      password_authentication
    else
      session[:return_to] = request.env["HTTP_REFERER"]
    end
  end

  def signup
    @user = User.new(params.fetch(:user, {}).permit(:login, :password, :email, :password_confirmation))
    return unless request.post?
    @user.save!
    self.current_user = @user
    redirect_back_or_default(:controller => :home)
    flash[:notice] = _("Thanks for signing up!")
  rescue ActiveRecord::RecordInvalid
    render :action => 'signup'
  end

  def google_signin
    auth = request.env["omniauth.auth"]

    self.current_user = User.where(openid_connect_token: auth[:uid]).first
    if not self.current_user
      self.current_user = User.where(identity_url: auth[:extra][:id_info][:openid_id]).first
      if not self.current_user
        self.current_user = User.create(openid_connect_token: auth[:uid])
      end
      if self.current_user.name.nil?
        self.current_user.name = auth[:info][:name]
      end
    end

    successful_login
  end

  def failure
    failed_login _("Username or password invalid")
    redirect_to(controller: :account, action: :login)
  end

  def logout
    self.current_user.forget_me if logged_in?
    cookies.delete :auth_token
    reset_session
    flash[:notice] = _("You have been logged out.")
    redirect_to(:back) rescue redirect_to("/")
  end

  private

  def password_authentication
    self.current_user = User.authenticate(@user.login, @user.password)
    if logged_in?
      successful_login
    else
      failed_login _("Username or password invalid")
    end
  end

  def open_id_authentication
    @user.identity_url = params[:openid_url]
    authenticate_with_open_id do |result, identity_url|
      if result.successful?
        if self.current_user = User.find_or_create_by(identity_url: identity_url)
          successful_login
        else
          failed_login _("Sorry, no user by that identity URL exists (%{identity_url})" % { :identity_url => identity_url })
        end
      else
        failed_login result.message
      end
    end
  rescue
    failed_login $!.to_s
  end

  def successful_login
    if params[:remember_me] == "1"
      self.current_user.remember_me
      cookies[:auth_token] = { :value => self.current_user.remember_token , :expires => self.current_user.remember_token_expires_at }
    end
    current_user.last_login = Time.now
    current_user.save!
    redirect_back_or_default(:controller => :user, :action => :show, :id => current_user.id)
    flash[:notice] = _("Logged in successfully")
  end

  def failed_login(message)
    flash[:notice] = message
  end
end
