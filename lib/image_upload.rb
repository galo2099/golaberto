module ImageUpload
  include Magick
  def uploaded_image(l, type, filter_background = false)
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
    background = ImageList.new("#{Rails.root}/public/images/logos/100.png")
    image = background.composite(image, CenterGravity, OverCompositeOp)
    image.write("#{Rails.root}/public/images/#{type}/#{self.id}_100.png")
    image.scale!(15, 15)
    image.write("#{Rails.root}/public/images/#{type}/#{self.id}_15.png")
  end

 protected
  def crop_logo(image)
    cols = image.columns
    rows = image.rows
    x_left = 0
    x_right = cols - 1
    y_top = 0
    y_bottom = rows - 1
    cols.times do |i|
      if image.get_pixels(i, 0, 1, rows).select{|p| p.opacity != MaxRGB}.size > 0
        x_left = i
        break
      end
    end
    (cols - 1).downto(0) do |i|
      if image.get_pixels(i, 0, 1, rows).select{|p| p.opacity != MaxRGB}.size > 0
        x_right = i + 1
        break
      end
    end
    rows.times do |i|
      if image.get_pixels(0, i, cols, 1).select{|p| p.opacity != MaxRGB}.size > 0
        y_top = i
        break
      end
    end
    (rows - 1).downto(0) do |i|
      if image.get_pixels(0, i, cols, 1).select{|p| p.opacity != MaxRGB}.size > 0
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
