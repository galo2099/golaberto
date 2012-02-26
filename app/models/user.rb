require 'digest/sha1'
require 'image_upload'
class User < ActiveRecord::Base
  include ImageUpload

  cattr_accessor :current_user
  # Virtual attribute for the unencrypted password
  attr_accessor :password
  N_("User|Password")

  has_and_belongs_to_many :roles
  has_many :comments, :dependent => :destroy, :order => 'created_at ASC'
  has_many :game_edits, :class_name => "Game::Version", :foreign_key => "updater_id"

  validates_presence_of     :login, :email, :if => :not_openid?
  validates_presence_of     :password,                   :if => :password_required?
  validates_presence_of     :password_confirmation,      :if => :password_required?
  validates_length_of       :password, :within => 4..40, :if => :password_required?
  validates_confirmation_of :password,                   :if => :password_required?
  validates_length_of       :login,    :within => 3..40, :if => :not_openid?
  validates_length_of       :email,    :within => 3..100, :if => :not_openid?
  validates_uniqueness_of   :login, :email, :case_sensitive => false, :allow_nil => true
  validates_length_of       :name, :maximum => 30, :allow_blank => true
  validates_length_of       :location, :maximum => 100, :allow_blank => true
  validates_length_of       :about_me, :maximum => 2000, :allow_blank => true

  before_save :encrypt_password
  after_create :create_logo
  after_create :add_initial_roles

  # Authenticates a user by their login name and unencrypted password.  Returns the user or nil.
  def self.authenticate(login, password)
    u = find_by_login(login) # need to get the salt
    u && u.authenticated?(password) ? u : nil
  end

  # Encrypts some data with the salt.
  def self.encrypt(password, salt)
    Digest::SHA1.hexdigest("--#{salt}--#{password}--")
  end

  # has_role? simply needs to return true or false whether a user has a role or not.
  # It may be a good idea to have "admin" roles return true always
  def has_role?(role_in_question)
    @_list ||= self.roles.collect(&:name)
    return true if @_list.include?("admin")
    (@_list.include?(role_in_question.to_s) )
  end

  def can_comment?
    has_role?("commenter")
  end

  def can_edit?
    has_role?("editor")
  end

  # Encrypts the password with the user salt
  def encrypt(password)
    self.class.encrypt(password, salt)
  end

  def authenticated?(password)
    crypted_password == encrypt(password)
  end

  def remember_token?
    remember_token_expires_at && Time.now.utc < remember_token_expires_at
  end

  # These create and unset the fields required for remembering users between browser closes
  def remember_me
    self.remember_token_expires_at = 2.weeks.from_now.utc
    self.remember_token            = encrypt("#{email}--#{remember_token_expires_at}")
    save(false)
  end

  def forget_me
    self.remember_token_expires_at = nil
    self.remember_token            = nil
    save(false)
  end

  def display_login
    login or identity_url
  end

  def display_name
    name or display_login
  end

  def small_logo
    "#{id}_15.png"
  end

  def large_logo
    "#{id}_100.png"
  end

  def uploaded_picture(l, filter_background = false)
    uploaded_image(l, "users", filter_background)
  end

  protected
    # before filter
    def encrypt_password
      return if password.blank?
      self.salt = Digest::SHA1.hexdigest("--#{Time.now.to_s}--#{login}--") if new_record?
      self.crypted_password = encrypt(password)
    end

    def password_required?
      not_openid? && (crypted_password.blank? or not password.blank?)
    end

    def not_openid?
      identity_url.blank?
    end

    def create_logo
      icon = Quilt::Identicon.new display_login, :size => 15
      icon.write("#{RAILS_ROOT}/public/images/users/#{self.id}_15.png")
      icon = Quilt::Identicon.new display_login, :size => 100
      icon.write("#{RAILS_ROOT}/public/images/users/#{self.id}_100.png")
    end

    def add_initial_roles
      self.roles << Role.find_or_create_by_name("commenter")
    end
end
