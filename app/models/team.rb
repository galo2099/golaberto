class Team < ActiveRecord::Base
  require 'RMagick'
  include Magick

  has_many :comments, :as => :commentable, :dependent => :destroy, :order => 'created_at ASC'
  has_many :team_groups, :dependent => :delete_all
  has_many :groups, :through => :team_groups
  has_many :home_games, :foreign_key => "home_id", :class_name => "Game", :dependent => :destroy
  has_many :away_games, :foreign_key => "away_id", :class_name => "Game", :dependent => :destroy
  has_many :team_players, :dependent => :delete_all, :include => :player
  validates_length_of :name, :within => 1..40
  validates_length_of :country, :within => 1..40
  validates_uniqueness_of :name, :message => "already exists"

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: name , SQL Definition:varchar(255)
  # Field: country , SQL Definition:varchar(255)
  # Field: logo , SQL Definition:varchar(255)
  
  def uploaded_logo(l, filter_background = false)
    image = ImageList.new.from_blob(l.read)
    if filter_background
      # make the background transparent
      image = remove_background(image)
    end
    # crop the image to have only the logo
    image = crop_logo(image)
    image.change_geometry("100x100") do |cols, rows, img|
      img.resize!(cols, rows)
    end
    background = ImageList.new("#{RAILS_ROOT}/public/images/logos/100.png")
    image = background.composite(image, CenterGravity, OverCompositeOp) 
    image.write("#{RAILS_ROOT}/public/images/logos/#{self.id}_100.png")
    image.scale!(15, 15)
    image.write("#{RAILS_ROOT}/public/images/logos/#{self.id}_15.png")
    self.logo = "#{self.id}.svg"
    save!
  end

  def small_logo
    if logo.nil?
      '15.png'
    else
      logo.gsub(/(.*)\.svg/, '\1_15.png')
    end
  end

  def large_logo
    if logo.nil?
      '100.png'
    else
      logo.gsub(/(.*)\.svg/, '\1_100.png')
    end
  end
  
  def next_n_games(n, phase = nil)
    cond = [ "played = ?", false ]
    if phase.nil?
      cond[0] << " AND championships.category_id = ?";
      cond << Category::DEFAULT_CATEGORY
    else
      cond[0] << " AND phase_id = ?"
      cond << phase.id
    end
    ret = n_games(n, cond, "ASC").sort do |a,b|
      a.date <=> b.date
    end
    ret.slice(0..4)
  end

  def last_n_games(n, phase = nil)
    cond = [ "played = ?", true ]
    if phase.nil?
      cond[0] << " AND championships.category_id = ?";
      cond << Category::DEFAULT_CATEGORY
    else
      cond[0] << " AND phase_id = ?"
      cond << phase.id
    end
    ret = n_games(n, cond, "DESC").sort do |a,b|
      b.date <=> a.date
    end
    ret.slice(0..4)
  end

  private
  def n_games(n, condition, order)
    home = self.home_games.find(
        :all,
        :order => "date #{order}",
        :conditions => condition,
        :include => [ { :phase => :championship } ],
        :limit => n)
    away = self.away_games.find(
        :all,
        :order => "date #{order}",
        :conditions => condition,
        :include => [ { :phase => :championship } ],
        :limit => n)

    home + away
  end

  def crop_logo(image)
    cols = image.columns
    rows = image.rows
    x_left = 0
    x_right = cols - 1
    y_top = 0
    y_bottom = rows - 1
    transparent = Pixel.new(0, 0, 0, MaxRGB)
    cols.times do |i|
      if image.get_pixels(i, 0, 1, rows).select{|p| p != transparent}.size > 0
        x_left = i
        break
      end
    end
    (cols - 1).downto(0) do |i|
      if image.get_pixels(i, 0, 1, rows).select{|p| p != transparent}.size > 0
        x_right = i + 1
        break
      end
    end
    rows.times do |i|
      if image.get_pixels(0, i, cols, 1).select{|p| p != transparent}.size > 0
        y_top = i
        break
      end
    end
    (rows - 1).downto(0) do |i|
      if image.get_pixels(0, i, cols, 1).select{|p| p != transparent}.size > 0
        y_bottom = i + 1
        break
      end
    end
    # sanitize values
    x_left = x_right if x_left > x_right
    y_top = y_bottom if y_top > y_bottom
    image.crop(x_left, y_top, x_right - x_left, y_bottom - y_top)
  end
  
  def remove_background(image)
    image.fuzz = "5%"
    cols = image.columns
    rows = image.rows
    background_pixel = image.get_pixels(0, 0, 1, 1).first
    if background_pixel.opacity != MaxRGB
      rows.times do |row|
        if image.get_pixels(0, row, 1, 1).first == background_pixel
          image = image.matte_floodfill(0, row)
        end
        if image.get_pixels(cols - 1, row, 1, 1).first == background_pixel
          image = image.matte_floodfill(cols - 1, row)
        end
      end
      cols.times do |col|
        if image.get_pixels(col, 0, 1, 1).first == background_pixel
          image = image.matte_floodfill(col, 0)
        end
        if image.get_pixels(col, rows - 1, 1, 1).first == background_pixel
          image = image.matte_floodfill(col, rows - 1)
        end
      end
    end
    image
  end
end
